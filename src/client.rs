use bevy::{color::palettes::css::*, prelude::*};
use bevy_prototype_lyon::prelude::*;
use rand::{Rng};
use std::collections::VecDeque;
use bevy::window::PrimaryWindow;

fn main() {
    let map = Map::new();
    let worm = Worm::new(map.radius);

    App::new()
        .add_plugins((DefaultPlugins, ShapePlugin))
        .insert_resource(ClearColor(Color::srgb(0.8, 0.3, 0.3)))
        .insert_resource(map)
        .insert_resource(worm)
        .insert_resource(Dots::new())
        .insert_resource(AbsorbingDots::default())
        .insert_resource(RemoteWorms::new())
        .insert_resource(Leaderboard::new(5))
        .add_systems(Startup, setup)
        .add_systems(Update, (
            input_dir,
            move_head,
            redraw_worm,
            check_collision,
            animate_absorbing,
            check_damage_zone,
            handle_reset,
            check_player_death,
            mouse_aim,
            draw_leaderboard_ui,
            camera_follow))
        .add_systems(FixedUpdate, (move_head, check_collision, check_damage_zone, check_player_death, update_leaderboard))
        .run();
}

#[derive(Resource, Default)]
struct AbsorbingDots {
    // entity, growth, elapsed, duration, start_pos
    items: Vec<(Entity, usize, f32, f32, Vec2)>,
}

fn camera_follow(
    worm: Res<Worm>,
    mut camera_q: Query<&mut Transform, With<Camera>>,
    time: Res<Time>,
) {
    if worm.is_dead {
        return;
    }

    if let Ok(mut transform) = camera_q.single_mut() {
        // smooth translation towards head using time-based exponential smoothing
        let dt = time.delta_secs();
        let target = Vec3::new(worm.head.x, worm.head.y, transform.translation.z);
        let trans_alpha = 1.0 - (-8.0 * dt).exp(); // responsiveness
        transform.translation = transform.translation.lerp(target, trans_alpha);

        // smooth zoom out as worm grows so map remains visible (gentle, time-based)
        let len = worm.points.len() as f32;
        let target_zoom = (1.0 + len * 0.005).clamp(1.0, 3.0);
        let zoom_alpha = 1.0 - (-3.0 * dt).exp();
        transform.scale = transform.scale.lerp(Vec3::splat(target_zoom), zoom_alpha);
    }
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
            radius: 2500.0, 
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

    is_dead: bool,
    // --- 추가: 회전 관련 파라미터
    turn_speed: f32,
    damage_accumulator: f32,
}

#[derive(Component)]
struct Dot {
    growth: usize,
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
    // sector (fan) absorption params - made larger by default
    const SECTOR_RADIUS: f32 = 120.0;
    const SECTOR_ANGLE: f32 = std::f32::consts::FRAC_PI_2;

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
        // Random growth amount 1~3
        let growth = rand::rng().random_range(1..=3);
        
        // Size scales with growth: 1->1.0x, 2->1.2x, 3->1.4x
        let radius = Self::DOT_RADIUS * (0.8 + 0.2 * growth as f32);

        let circle = shapes::Circle {
            radius,
            center: Vec2::ZERO,
        };

        let entity = commands.spawn((
            ShapeBuilder::with(&circle).fill(PURPLE).build(),
                // render dots between worm and damage zone
                Transform::from_translation(pos.extend(0.0)),
            DotsShape,
            Dot { growth },
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

    /// Remove dots that lie inside a sector (fan) in front of `head` along `dir`.
    /// Returns removed entities.
    fn remove_in_sector(&mut self, head: Vec2, dir: Vec2) -> Vec<Entity> {
        // default wrapper uses the constants
        self.remove_in_sector_params(head, dir, Self::SECTOR_RADIUS, Self::SECTOR_ANGLE)
    }

    fn remove_in_sector_params(&mut self, head: Vec2, dir: Vec2, radius: f32, angle: f32) -> Vec<Entity> {
        let mut removed = Vec::new();

        let half_cos = (angle * 0.5).cos();

        self.items.retain(|(pos, entity)| {
            let rel = *pos - head;
            let dist = rel.length();
            if dist <= std::f32::EPSILON {
                removed.push(*entity);
                return false;
            }
            if dist > radius {
                return true;
            }

            let dir_dot = dir.dot(rel / dist);
            if dir_dot >= half_cos {
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
    const GROWTH_PER_DOT: usize = 1;
    const MIN_POINTS: usize = 16;
    const INITIAL_MAX_POINTS: usize = Self::MIN_POINTS;

    fn new(map_radius: f32) -> Self {
        let mut rng = rand::rng();
        let id = rng.random();
        let sample_distance = 6.0;

        let angle = rng.random_range(0.0..std::f32::consts::TAU);
        let dir = Dir2::new(Vec2::new(angle.cos(), angle.sin())).unwrap();

        let spawn_radius = map_radius * 0.8;
        let body_length = sample_distance * Self::INITIAL_MAX_POINTS as f32;
        let safe_radius = spawn_radius - body_length;
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

            is_dead: false,
        }
    }

    fn reset(&mut self, map_radius: f32) {
        let mut rng = rand::rng();

        let angle = rng.random_range(0.0..std::f32::consts::TAU);
        let dir = Dir2::new(Vec2::new(angle.cos(), angle.sin())).unwrap();

        let spawn_radius = map_radius * 0.8;
        let body_length = self.sample_distance * Self::INITIAL_MAX_POINTS as f32;
        let safe_radius = spawn_radius - body_length;
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
        self.is_dead = false;
    }
    
    fn grow(&mut self, points: usize) {
        self.max_points += points * Self::GROWTH_PER_DOT;
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

fn mouse_aim(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    mut worm: ResMut<Worm>,
) {
    if worm.is_dead {
        return;
    }

    if !mouse.pressed(MouseButton::Left) {
        return;
    }

    // single()은 Result를 줌 → Ok일 때만 Window를 꺼내기
    let Ok(window) = windows.single() else {
        return;
    };

    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };

    // camera도 single()은 Result → Ok일 때만 꺼내기
    let Ok((camera, cam_tf)) = camera_q.single() else {
        return;
    };

    let Ok(world_pos) = camera.viewport_to_world_2d(cam_tf, cursor_pos) else {
        return;
    };

    let to_mouse = world_pos - worm.head;

    if to_mouse.length_squared() < 0.0001 {
        return;
    }

    worm.target_dir = Dir2::new(to_mouse).unwrap();
}

#[derive(Component)]
struct WormShape;

#[derive(Component)]
struct WormCap {
    is_head: bool,
}

#[derive(Component)]
struct DamageZone {
    radius: f32,
    damage_per_sec: f32,
}

fn setup(mut commands: Commands, mut dots: ResMut<Dots>, map: Res<Map>, worm: Res<Worm>) {
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
        // place damage zone below dots but above background
        Transform::from_translation(pos.extend(-0.9)),
        DamageZone {
            radius,
            damage_per_sec: 30.0,
        },
    ));

    // 초기 더미 path: 실제 게임 시작 시에는 `worm` 리소스의 위치로 초기화
    let path = ShapePath::new().move_to(worm.head).line_to(worm.head + Vec2::new(1.0, 0.0));

    // main body (place above dots)
    commands
        .spawn((ShapeBuilder::with(&path).stroke((GREEN, 12.0)).build(), Transform::from_translation(Vec3::new(0.0, 0.0, 0.5))))
        .insert(WormShape);

    // caps: head + tail (spawn as separate entities but mark with WormShape so kill() clears them)
    let cap_circle = shapes::Circle { radius: 6.0, center: Vec2::ZERO };
    // head cap
    commands.spawn((
        ShapeBuilder::with(&cap_circle).fill(GREEN).build(),
        Transform::from_translation(worm.head.extend(0.5)),
        WormShape,
        WormCap { is_head: true },
    ));
    // tail cap (just behind head initially)
    commands.spawn((
        ShapeBuilder::with(&cap_circle).fill(GREEN).build(),
        Transform::from_translation((worm.head - Vec2::new(6.0, 0.0)).extend(0.5)),
        WormShape,
        WormCap { is_head: false },
    ));

    // dots 생성 (positions 리스트에 추가 + Entity 생성)
    for _ in 0..200 {
        dots.spawn(&mut commands, map.radius);
    }

    commands.spawn((
        Text::new("Leaderboard"),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(16.0),
            top: Val::Px(16.0),
            ..default()
        },
        LeaderboardText,
    ));
}

fn draw_leaderboard_ui(
    worm: Res<Worm>,
    leaderboard: Res<Leaderboard>,
    mut q: Query<&mut Text, With<LeaderboardText>>,
) {
    if !leaderboard.is_changed() {
        return;
    }

    let my_rank = leaderboard.my_rank(worm.id);

    if let Some(mut text) = q.iter_mut().next() {
        let mut s = String::from("Leaderboard\n");

        for (i, e) in leaderboard.entries.iter().enumerate() {
            let me_mark = if e.is_me { " (ME)" } else { "" };
            s.push_str(&format!("{}. {} - {}{}\n", i + 1, e.id, e.length, me_mark));
        }

        match my_rank {
            Some(r) => s.push_str(&format!("\nMy Rank: {}", r)),
            None => s.push_str("\nMy Rank: -"),
        }

        *text = Text::new(s);
    }
}

/// 방향 전환(키 입력). (WASD / 화살표)
fn input_dir(keys: Res<ButtonInput<KeyCode>>, time: Res<Time>, mut worm: ResMut<Worm>) {
    if worm.is_dead {
        return;
    }

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
    map: Res<Map>,
    worm_query: Query<Entity, With<WormShape>>,
) {
    if keys.just_pressed(KeyCode::KeyR) {
        // 기존 지렁이 몸통 삭제
        for entity in worm_query.iter() {
            commands.entity(entity).despawn();
        }
        
        // 지렁이 데이터 리셋
        worm.reset(map.radius);
        
        // 새로운 지렁이 몸통 생성 (main + caps)
        let path = ShapePath::new().move_to(worm.head).line_to(worm.head + Vec2::new(1.0, 0.0));
        commands
            .spawn((ShapeBuilder::with(&path).stroke((GREEN, 12.0)).build(), Transform::from_translation(Vec3::new(0.0, 0.0, 0.5))))
                .insert(WormShape);

        let cap_circle = shapes::Circle { radius: 6.0, center: Vec2::ZERO };
        commands.spawn((
            ShapeBuilder::with(&cap_circle).fill(GREEN).build(),
            Transform::from_translation(worm.head.extend(0.5)),
            WormShape,
            WormCap { is_head: true },
        ));
        commands.spawn((
            ShapeBuilder::with(&cap_circle).fill(GREEN).build(),
            Transform::from_translation((worm.head - Vec2::new(6.0, 0.0)).extend(0.5)),
            WormShape,
            WormCap { is_head: false },
        ));
    }
}

/// 머리를 시간 기반으로 이동시키고, 일정 거리마다 points에 기록
fn move_head(time: Res<Time>, mut worm: ResMut<Worm>) {
    if worm.is_dead {
        return;
    }

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
fn redraw_worm(
    worm: Res<Worm>,
    mut main_shape_q: Query<&mut Shape, (With<WormShape>, Without<WormCap>)>,
    mut cap_q: Query<(&mut Shape, &mut Transform, &WormCap), With<WormShape>>,
) {
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

    // thickness scales with length; doubled baseline and growth
    let thickness = (16.0 + worm.points.len() as f32 * 0.24).clamp(16.0, 72.0);

    // 2) Shape 교체로 렌더 반영 (main body)
    if let Some(mut shape) = main_shape_q.iter_mut().next() {
        *shape = ShapeBuilder::with(&path).stroke((GREEN, thickness)).build();
    }

    // update caps: head & tail as filled circles
    let head_pos = *pts.last().unwrap();
    let tail_pos = pts.first().copied().unwrap_or(head_pos);
    let cap_radius = thickness * 0.5;

    for (mut shape, mut tf, cap) in cap_q.iter_mut() {
        let circle = shapes::Circle { radius: cap_radius, center: Vec2::ZERO };
        *shape = ShapeBuilder::with(&circle).fill(GREEN).build();
        if cap.is_head {
            tf.translation = head_pos.extend(0.5);
        } else {
            tf.translation = tail_pos.extend(0.5);
        }
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
    dot_query: Query<&Dot>,
    dot_tf_q: Query<&Transform, With<DotsShape>>,
    mut absorbing: ResMut<AbsorbingDots>,
) {
    if worm.is_outside(&map) {
        worm.kill(&mut commands, &worm_query);
        worm.is_dead = true;
        return;
    }

    let thickness = (16.0 + worm.points.len() as f32 * 0.24).clamp(16.0, 72.0);
    // sector center: a bit in front of the head (half thickness)
    let sector_center = worm.head + worm.dir.as_vec2() * (thickness * 0.5);
    // radius: ~150% of thickness
    let radius = thickness * 1.50;
    // angle: 150 degrees (5π/6)
    let angle = std::f32::consts::PI * 5.0 / 6.0;

    let removed_entities = dots.remove_in_sector_params(sector_center, worm.dir.as_vec2(), radius, angle);

    if !removed_entities.is_empty() {
        for entity in removed_entities.into_iter() {
            let growth = dot_query.get(entity).map(|d| d.growth).unwrap_or(1);
            let start_pos = dot_tf_q.get(entity).map(|t| t.translation.truncate()).unwrap_or(Vec2::ZERO);
            let duration = 0.18;
            absorbing.items.push((entity, growth, 0.0, duration, start_pos));
        }
    }
}

fn animate_absorbing(
    mut commands: Commands,
    time: Res<Time>,
    mut absorbing: ResMut<AbsorbingDots>,
    mut worm: ResMut<Worm>,
    mut transforms: Query<&mut Transform, With<DotsShape>>,
    mut dots: ResMut<Dots>,
    map: Res<Map>,
) {
    let dt = time.delta_secs();

    if absorbing.items.is_empty() {
        return;
    }

    let mut remaining = Vec::new();

    for (entity, growth, mut elapsed, duration, start_pos) in absorbing.items.drain(..) {
        elapsed += dt;
        let t = (elapsed / duration).clamp(0.0, 1.0);
        let target = worm.head;
        let pos = start_pos.lerp(target, t);

        if let Ok(mut tf) = transforms.get_mut(entity) {
            // keep dot layer between worm and damage zone while animating
            tf.translation = pos.extend(0.0);
            let scale = 1.0 - t * 0.9;
            tf.scale = Vec3::splat(scale.max(0.05));
        }

        if elapsed >= duration {
            // finalize: despawn entity, add growth, spawn replacement
            commands.entity(entity).despawn();
            worm.grow(growth);
            // spawn replacement
            dots.spawn(&mut commands, map.radius);
        } else {
            remaining.push((entity, growth, elapsed, duration, start_pos));
        }
    }

    absorbing.items = remaining;
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
    _map: Res<Map>,
    remote: Res<RemoteWorms>,
    worm_query: Query<Entity, With<WormShape>>,
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
    worm.kill(&mut commands, &worm_query); // Need to despawn body
    worm.is_dead = true;
}

#[derive(Debug, Clone)]
struct LeaderboardEntry {
    id: u64,       // 유저 ID
    length: usize, // 길이(점수)
    is_me: bool,   // 내 캐릭터인지
}

#[derive(Resource, Debug)]
struct Leaderboard {
    top_n: usize,                  // 상위 몇 명까지 보관할지
    entries: Vec<LeaderboardEntry> // 계산된 순위 결과
}

impl Leaderboard {
    fn new(top_n: usize) -> Self {
        Self {
            top_n,
            entries: Vec::new(),
        }
    }

    // 나중에 UI팀이 "내 등수" 필요할 때 바로 쓰라고 넣어둔 보조 함수
    fn my_rank(&self, my_id: u64) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| e.id == my_id)
            .map(|idx| idx + 1)
    }
}

fn update_leaderboard(
    worm: Res<Worm>,
    remote: Res<RemoteWorms>,
    mut leaderboard: ResMut<Leaderboard>,
) {
    let mut list = Vec::with_capacity(1 + remote.worms.len());

    // 내 플레이어
    list.push(LeaderboardEntry {
        id: worm.id,
        length: worm.points.len(), // 현재 몸통 샘플 길이 기준
        is_me: true,
    });

    // 다른 플레이어들
    for rw in remote.worms.iter() {
        // 혹시 내 ID가 섞여 들어오면 중복 방지
        if rw.id == worm.id {
            continue;
        }

        list.push(LeaderboardEntry {
            id: rw.id,
            length: rw.points.len(),
            is_me: false,
        });
    }

    // 정렬: 길이 내림차순(큰 게 1등), 동점이면 ID 오름차순
    list.sort_by(|a, b| b.length.cmp(&a.length).then(a.id.cmp(&b.id)));

    // top_n만 유지
    let top_n = leaderboard.top_n;
    if list.len() > top_n {
        list.truncate(top_n);
    }

    // 결과 저장
    leaderboard.entries = list;
}

#[derive(Component)]
struct LeaderboardText;

#[derive(Component)]
struct DamageText;
