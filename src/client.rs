use bevy::{color::palettes::css::*, prelude::*};
use bevy_prototype_lyon::prelude::*;
use rand::{Rng};
use std::collections::VecDeque;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, ShapePlugin))
        .insert_resource(ClearColor(Color::srgb(0.8, 0.3, 0.3)))
        .insert_resource(Map::new())
        .insert_resource(Worm::new())
        .insert_resource(Dots::new())
        .insert_resource(RemoteWorms::new())
        .add_systems(Startup, setup)
        .add_systems(Update, (input_dir, move_head, redraw_worm, check_collision, check_damage_zone, handle_reset, check_player_death))
        .run();
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum SpeedMode {
    Nomal,
    Boost
}

#[derive(Resource)]
struct Map {
    radius: f32,
}

impl Map {
    fn new() -> Self {
        Self {
            radius: 400.0, 
        }
    }

    fn random_circle_inside(&self, min_radius: f32, max_radius: f32) -> (Vec2, f32) {
        let mut rng = rand::rng();

        let effective_max = max_radius.min(self.radius);
        let effective_min = min_radius.min(effective_max);

        let circle_radius = rng.random_range(effective_min..=effective_max);

        let max_distance = self.radius - circle_radius;

        let position = if max_distance <= 0.0 {
            Vec2::ZERO
        } else {
            let r = rng.random_range(0.0..1.0f32).sqrt() * max_distance;
            let theta = rng.random_range(0.0..std::f32::consts::TAU);
            Vec2::new(r * theta.cos(), r * theta.sin())
        };

        (position, circle_radius)
    }
}

#[derive(Resource)]
struct Worm {
    id: u64,
    head: Vec2,
     // --- 변경: Vec2 -> Dir2 (항상 "정규화된 방향"만 들고 있게)
    dir: Dir2,

    // --- 추가: 목표 방향(키를 누르는 동안 이 값이 계속 돌아감)
    target_dir: Dir2,
    
    mode: SpeedMode, // 현재 속도 모드
    base_speed: f32,
    boost_speed: f32,

    boost_min: f32, // 남은 부스트 시간
    boost_max: f32, // 최대 부스트 시간
    boost_recharge: f32, // 부스트 회복 속도
    boost_available: bool, // 완전 충전 전까지 재사용 금지

    points: VecDeque<Vec2>,    // 머리 위치 히스토리 = 몸통
    max_points: usize,         // 몸 길이 (샘플 수)
    sample_distance: f32,      // 이 거리 이상 이동해야 points에 추가
    // --- 추가: 회전 관련 파라미터
    turn_speed: f32,
    damage_accumulator: f32,
}

#[derive(Component)]
struct DotsShape;

#[derive(Resource)]
struct Dots {
    items: Vec<(Vec2, Entity)>,  // Position + Entity 함께!
}

impl Dots {
    const EAT_RADIUS: f32 = 20.0;
    const DOT_RADIUS: f32 = 12.0;

    fn new() -> Self {
        Self {
            items: Vec::new(),
        }
    }

    fn random_position(map_radius: f32) -> Vec2 {
        let mut rng = rand::rng();
        
        let half_radius = map_radius * 0.5;
        
        let r = if rng.random_bool(0.5) {
            rng.random_range(0.0..half_radius)
        } else {
            rng.random_range(half_radius..map_radius)
        };
        
        let theta = rng.random_range(0.0..std::f32::consts::TAU);
        Vec2::new(r * theta.cos(), r * theta.sin())
    }

    /// Spawn a dot at random position
    fn spawn(&mut self, commands: &mut Commands, map_radius: f32) {
        let pos = Self::random_position(map_radius);
        self.spawn_at(commands, pos);
    }

    /// 원하는 위치에 점(도트) 하나를 생성한다. (죽으면 내 몸통을 점으로 바꿀 때 필요)
    fn spawn_at(&mut self, commands: &mut Commands, pos: Vec2) -> Entity { // [변경됨] 추가
        let circle = shapes::Circle {
            radius: Self::DOT_RADIUS,
            center: Vec2::ZERO,
        };

        let entity = commands.spawn((
            ShapeBuilder::with(&circle).fill(PURPLE).build(),
            Transform::from_translation(pos.extend(0.0)),
            DotsShape,
        )).id();

        self.items.push((pos, entity));
        entity
    }

    fn remove_nearby(&mut self, center: Vec2) -> Vec<Entity> {
        let mut removed = Vec::new();
        self.items.retain(|(pos, entity)| {
            if pos.distance(center) <= Self::EAT_RADIUS {
                removed.push(*entity);
                false
            } else {
                true
            }
        });
        removed
    }
}

impl Worm {
    const GROWTH_PER_DOT: usize = 20;
    const MIN_POINTS: usize = 5;
    const INITIAL_MAX_POINTS: usize = Self::MIN_POINTS;
    const SPAWN_RADIUS: f32 = 300.0;

    fn new() -> Self {
        let mut rng = rand::rng();
        let id = rng.random();
        let sample_distance = 6.0;

        let angle = rng.random_range(0.0..std::f32::consts::TAU);
        let dir = Dir2::new(Vec2::new(angle.cos(), angle.sin())).unwrap();

        let body_length = sample_distance * Self::INITIAL_MAX_POINTS as f32;
        let safe_radius = Self::SPAWN_RADIUS - body_length;
        let r = rng.random_range(0.0..1.0f32).sqrt() * safe_radius.max(0.0);
        let pos_angle = rng.random_range(0.0..std::f32::consts::TAU);
        let head = Vec2::new(r * pos_angle.cos(), r * pos_angle.sin());

        let mut points = VecDeque::new();
        for i in (0..Self::INITIAL_MAX_POINTS).rev() {
            let offset = dir.as_vec2() * (-sample_distance * i as f32);
            points.push_back(head + offset);
        }

        Self {
            id,
            head,
            dir,
            target_dir: dir, // --- 추가: 처음엔 목표도 현재 방향과 동일
            mode: SpeedMode::Nomal,
            base_speed: 220.0,
            boost_speed: 350.0,

            boost_min:3.0,
            boost_max:3.0,
            boost_recharge: 0.4,
            boost_available: true,

            points,
            max_points: Self::INITIAL_MAX_POINTS,
            sample_distance,
            turn_speed: 3.0,
            damage_accumulator: 0.0,
        }
    }

    fn reset(&mut self) {
        let mut rng = rand::rng();

        let angle = rng.random_range(0.0..std::f32::consts::TAU);
        let dir = Dir2::new(Vec2::new(angle.cos(), angle.sin())).unwrap();

        let body_length = self.sample_distance * Self::INITIAL_MAX_POINTS as f32;
        let safe_radius = Self::SPAWN_RADIUS - body_length;
        let r = rng.random_range(0.0..1.0f32).sqrt() * safe_radius.max(0.0);
        let pos_angle = rng.random_range(0.0..std::f32::consts::TAU);
        let head = Vec2::new(r * pos_angle.cos(), r * pos_angle.sin());

        self.points.clear();
        for i in (0..Self::INITIAL_MAX_POINTS).rev() {
            let offset = dir.as_vec2() * (-self.sample_distance * i as f32);
            self.points.push_back(head + offset);
        }

        self.head = head;
        self.dir = dir;
        self.target_dir = dir;
        self.max_points = Self::INITIAL_MAX_POINTS;
        self.damage_accumulator = 0.0;
        self.mode = SpeedMode::Nomal;
        self.boost_min = self.boost_max;
        self.boost_available = true;
    }
    
    fn grow(&mut self, count: usize) {
        self.max_points += count * Self::GROWTH_PER_DOT;
    }

    fn take_damage(&mut self, amount: f32) -> bool {
        self.damage_accumulator += amount;
        
        while self.damage_accumulator >= 1.0 {
            self.damage_accumulator -= 1.0;
            
            if self.max_points > Self::MIN_POINTS {
                self.max_points -= 1;
                
                if self.points.len() > self.max_points {
                    self.points.pop_front();
                }
            }
        }

        self.max_points > Self::MIN_POINTS
    }

    fn is_outside(&self, map: &Map) -> bool {
        self.head.length() > map.radius
    }

    fn kill(&self, commands: &mut Commands, worm_query: &Query<Entity, With<WormShape>>) {
        for entity in worm_query.iter() {
            commands.entity(entity).despawn();
        }
    }
}

#[derive(Component)]
struct WormShape;

#[derive(Component)]
struct DamageZone {
    radius: f32,
    damage_per_sec: f32,
}

fn setup(mut commands: Commands, mut dots: ResMut<Dots>, map: Res<Map>) {
    commands.spawn(Camera2d);

    // 게임 맵 생성
    let inner_circle = shapes::Circle {
        radius: map.radius,
        center: Vec2::ZERO,
    };
    commands.spawn((
        ShapeBuilder::with(&inner_circle).fill(Color::srgb(0.1, 0.1, 0.15)).build(),
        Transform::from_translation(Vec3::new(0.0, 0.0, -1.0)),
    ));

    let light_blue_transparent = Color::srgba(0.5, 0.8, 1.0, 0.3);
    
    let (pos, radius) = map.random_circle_inside(30.0, 100.0);
    let circle = shapes::Circle {
        radius,
        center: Vec2::ZERO,
    };
    commands.spawn((
        ShapeBuilder::with(&circle).fill(light_blue_transparent).build(),
        Transform::from_translation(pos.extend(-0.5)),
        DamageZone {
            radius,
            damage_per_sec: 30.0,
        },
    ));

    // 초기 더미 path
    let path = ShapePath::new()
        .move_to(Vec2::new(-200.0, 0.0))
        .line_to(Vec2::new(-199.0, 0.0));

    commands
        .spawn(ShapeBuilder::with(&path).stroke((GREEN, 10.0)).build())
        .insert(WormShape);

    // dots 생성 (positions 리스트에 추가 + Entity 생성)
    for _ in 0..20 {
        dots.spawn(&mut commands, map.radius);
    }
}

/// 방향 전환(키 입력). (WASD / 화살표)
fn input_dir(keys: Res<ButtonInput<KeyCode>>, time: Res<Time>, mut worm: ResMut<Worm>) {
    let dt = time.delta_secs();

    // 부스트 키
    let boost_key = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);

    // 입력된 방향 벡터 계산
    let mut input_vec = Vec2::ZERO;

    if keys.pressed(KeyCode::ArrowUp) || keys.pressed(KeyCode::KeyW) {
        input_vec.y += 1.0;
    }
    if keys.pressed(KeyCode::ArrowDown) || keys.pressed(KeyCode::KeyS) {
        input_vec.y -= 1.0;
    }
    if keys.pressed(KeyCode::ArrowLeft) || keys.pressed(KeyCode::KeyA) {
        input_vec.x -= 1.0;
    }
    if keys.pressed(KeyCode::ArrowRight) || keys.pressed(KeyCode::KeyD) {
        input_vec.x += 1.0;
    }

    // 키 입력이 있으면 target_dir을 그 방향으로 설정
    if input_vec != Vec2::ZERO {
        if let Ok(new_target) = Dir2::new(input_vec) {
            worm.target_dir = new_target;
        }
    }

    // dir이 target_dir을 부드럽게 따라감 (slerp)
    let t = (worm.turn_speed * dt).clamp(0.0, 1.0);
    worm.dir = worm.dir.slerp(worm.target_dir, t);

    if boost_key && worm.boost_available && worm.boost_min > 0.0 {
        worm.mode = SpeedMode::Boost;

        // 남은 시간 줄이기
        worm.boost_min = (worm.boost_min - dt).max(0.0);
        if worm.boost_min <= 0.0 {
            worm.boost_available = false;
        }
    } else {
        // 부스트 OFF
        worm.mode = SpeedMode::Nomal;

        // 부스트 회복
        worm.boost_min = (worm.boost_min + worm.boost_recharge * dt).min(worm.boost_max);
        if worm.boost_min >= worm.boost_max {
            worm.boost_available = true;
        }
    }
}

/// 임시로 만든 리셋 함수
fn handle_reset(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut worm: ResMut<Worm>,
    worm_query: Query<Entity, With<WormShape>>,
) {
    if keys.just_pressed(KeyCode::KeyR) {
        // 기존 지렁이 몸통 삭제
        for entity in worm_query.iter() {
            commands.entity(entity).despawn();
        }
        
        // 지렁이 데이터 리셋
        worm.reset();
        
        // 새로운 지렁이 몸통 생성
        let path = ShapePath::new()
            .move_to(worm.head)
            .line_to(worm.head + Vec2::new(1.0, 0.0));
        
        commands
            .spawn(ShapeBuilder::with(&path).stroke((GREEN, 10.0)).build())
            .insert(WormShape);
    }
}

/// 머리를 시간 기반으로 이동시키고, 일정 거리마다 points에 기록
fn move_head(time: Res<Time>, mut worm: ResMut<Worm>) {
    let dt = time.delta_secs();

    // 속도는 부스트 모드로 설정
    let speed = match worm.mode {
        SpeedMode::Nomal => worm.base_speed,
        SpeedMode::Boost => worm.boost_speed,
    };

    // Dir2는 길이가 1인 "방향"이므로, as_vec2()로 Vec2를 꺼내서 위치 계산에 사용
    let new_head = worm.head + worm.dir.as_vec2() * speed * dt;

    // 샘플링: 너무 촘촘하면 점이 과도하게 늘어서 지렁이가 “굵은 덩어리”처럼 보일 수 있음
    let push = match worm.points.back().copied() {
        Some(last) => new_head.distance(last) >= worm.sample_distance,
        None => true,
    };

    worm.head = new_head;

    if push {
        worm.points.push_back(new_head);
        while worm.points.len() > worm.max_points {
            worm.points.pop_front();
        }
    }
}

/// points로 지렁이 몸통을 다시 그리고 Shape를 교체
fn redraw_worm(worm: Res<Worm>, mut query: Query<&mut Shape, With<WormShape>>) {
    // points가 안 바뀌었으면 스킵
    if !worm.is_changed() {
        return;
    }

    let pts: Vec<Vec2> = worm.points.iter().copied().collect();
    if pts.len() < 2 {
        return;
    }

    // 1) 일단 polyline (line_to)로 몸통 생성
    //    여기서 Catmull-Rom 샘플링을 넣으면 “진짜 스플라인” 느낌이 됩니다.
    let mut path = ShapePath::new().move_to(pts[0]);
    for p in pts.iter().skip(1) {
        path = path.line_to(*p);
    }

    // 2) Shape 교체로 렌더 반영
    if let Some(mut shape) = query.iter_mut().next() {
        *shape = ShapeBuilder::with(&path).stroke((GREEN, 10.0)).build();
    }
}

fn check_damage_zone(
    time: Res<Time>,
    mut worm: ResMut<Worm>,
    damage_zones: Query<(&Transform, &DamageZone)>,
) {
    let dt = time.delta_secs();
    let head = worm.head;

    for (transform, zone) in damage_zones.iter() {
        let zone_center = transform.translation.truncate();
        let distance = head.distance(zone_center);

        if distance <= zone.radius {
            let damage = zone.damage_per_sec * dt;
            worm.take_damage(damage);
        }
    }
}

fn check_collision(
    mut commands: Commands,
    mut worm: ResMut<Worm>,
    mut dots: ResMut<Dots>,
    map: Res<Map>,
    worm_query: Query<Entity, With<WormShape>>,
) {
    if worm.is_outside(&map) {
        worm.kill(&mut commands, &worm_query);
        return;
    }
    // 지렁이 머리와 가까운 점들을 제거
    let removed_entities = dots.remove_nearby(worm.head);
    let count = removed_entities.len();

    if count > 0 {
        // 제거된 점들을 삭제
        for entity in removed_entities {
            commands.entity(entity).despawn();
        }

        worm.grow(count);

        // 제거된 점들 만큼 새로운 점들 생성
        for _ in 0..count {
            dots.spawn(&mut commands, map.radius);
        }
    }
}

#[derive(Resource)]
struct RemoteWorms { 
    worms: Vec<RemoteWorm>,
}

impl RemoteWorms { 
    fn new() -> Self {
        Self { worms: Vec::new() }
    }
}

// 서버에서 받게 될 "다른 지렁이"의 상태(최소 정보만)
struct RemoteWorm { 
    id: u64,
    points: Vec<Vec2>, 
}

fn check_player_death( 
    mut commands: Commands,
    mut worm: ResMut<Worm>,
    mut dots: ResMut<Dots>,
    remote: Res<RemoteWorms>,
) {
    // 더미 없이 진행: 현재 원격 지렁이가 없으면 아무 일도 안 함.
    if remote.worms.is_empty() {
        return;
    }

    // 내 머리 위치
    let head = worm.head;

    // 충돌 여부
    let mut hit_other = false;

    for other in remote.worms.iter() {
        // 혹시 같은 ID면 무시 (내 데이터가 들어온 경우 대비)
        if other.id == worm.id {
            continue;
        }

        // 다른 지렁이 몸통 점들 중 하나라도 머리랑 가까우면 "충돌"
        let collided = other
            .points
            .iter()
            .any(|p| p.distance(head) <= Dots::EAT_RADIUS);

        if collided {
            hit_other = true;
            break;
        }
    }

    if !hit_other {
        return;
    }

    // 여기부터: "내 지렁이를 점으로 변환"
    // 너무 많은 점이 한 번에 생기면 화면이 지저분하니, 몸통 점을 몇 칸씩 건너뛰며 생성
    const STEP: usize = 5;

    for (i, pos) in worm.points.iter().copied().enumerate() {
        if i % STEP == 0 {
            dots.spawn_at(&mut commands, pos);
        }
    }

    // 죽었으니 내 지렁이 리셋(새 ID로 다시 시작)
    *worm = Worm::new();
}