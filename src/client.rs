use bevy::{color::palettes::css::*, prelude::*};
use bevy_prototype_lyon::prelude::*;
use rand::Rng;
use std::collections::VecDeque;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, ShapePlugin))
        .insert_resource(Worm::new())
        .insert_resource(Dots::new())
        .add_systems(Startup, setup)
        .add_systems(Update, (input_dir, move_head, redraw_worm, check_collision))
        .run();
}

#[derive(Resource)]
struct Worm {
    head: Vec2,
     // --- 변경: Vec2 -> Dir2 (항상 "정규화된 방향"만 들고 있게)
    dir: Dir2,

    // --- 추가: 목표 방향(키를 누르는 동안 이 값이 계속 돌아감)
    target_dir: Dir2,
    speed: f32,                // px/s
    points: VecDeque<Vec2>,    // 머리 위치 히스토리 = 몸통
    max_points: usize,         // 몸 길이 (샘플 수)
    sample_distance: f32,      // 이 거리 이상 이동해야 points에 추가
    // --- 추가: 회전 관련 파라미터
    turn_speed: f32,   // 초당 얼마나 돌지(각속도 느낌)
    turn_follow: f32,  // target_dir을 얼마나 빨리 따라갈지(부드러움 정도)
}

#[derive(Component)]
struct DotsShape;

#[derive(Resource)]
struct Dots {
    items: Vec<(Vec2, Entity)>,  // Position + Entity 함께!
}

impl Dots {
    const EAT_RADIUS: f32 = 20.0;
    const SPAWN_RADIUSX: f32 = 600.0;
    const SPAWN_RADIUSY: f32 = 300.0;
    const DOT_RADIUS: f32 = 12.0;

    fn new() -> Self {
        Self {
            items: Vec::new(),
        }
    }

    fn random_position() -> Vec2 {
        let mut rng = rand::rng();
        let x = rng.random_range(-Self::SPAWN_RADIUSX..Self::SPAWN_RADIUSX);
        let y = rng.random_range(-Self::SPAWN_RADIUSY..Self::SPAWN_RADIUSY);
        Vec2::new(x, y)
    }

    /// Spawn a dot at random position
    fn spawn(&mut self, commands: &mut Commands) {
        let pos = Self::random_position();

        let circle = shapes::Circle {
            radius: Self::DOT_RADIUS,
            center: Vec2::ZERO,
        };

        let entity = commands.spawn((
            ShapeBuilder::with(&circle).fill(RED).build(),
            Transform::from_translation(pos.extend(0.0)),
            DotsShape,
        )).id();

        self.items.push((pos, entity));
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

    fn new() -> Self {
        let head = Vec2::new(-200.0, 0.0);
        let mut points = VecDeque::new();
        points.push_back(head);

        // --- 초기 방향은 오른쪽
        let dir = Dir2::EAST;

        Self {
            head,
            dir,
            target_dir: dir, // --- 추가: 처음엔 목표도 현재 방향과 동일
            speed: 220.0,
            points,
            max_points: 120,
            sample_distance: 6.0,

            // --- 추가: 값은 취향에 따라 조정 가능
            turn_speed: 3.0,   // 클수록 더 빨리 회전
            turn_follow: 10.0, // 클수록 "목표 방향"을 더 빨리 따라감
        }
    }
    
    fn grow(&mut self, count: usize) {
        self.max_points += count * Self::GROWTH_PER_DOT;
    }
}

#[derive(Component)]
struct WormShape;

fn setup(mut commands: Commands, mut dots: ResMut<Dots>) {
    commands.spawn(Camera2d);

    // 초기 더미 path
    let path = ShapePath::new()
        .move_to(Vec2::new(-200.0, 0.0))
        .line_to(Vec2::new(-199.0, 0.0));

    commands
        .spawn(ShapeBuilder::with(&path).stroke((GREEN, 10.0)).build())
        .insert(WormShape);

    // dots 생성 (positions 리스트에 추가 + Entity 생성)
    for _ in 0..20 {
        dots.spawn(&mut commands);
    }
}

/// 방향 전환(키 입력). (WASD / 화살표)
fn input_dir(keys: Res<ButtonInput<KeyCode>>, time: Res<Time>, mut worm: ResMut<Worm>) {
    let dt = time.delta_secs();

    let mut turn = 0.0;

    if keys.pressed(KeyCode::ArrowLeft) || keys.pressed(KeyCode::KeyA) {
        turn += 1.0;
    }
    if keys.pressed(KeyCode::ArrowRight) || keys.pressed(KeyCode::KeyD) {
        turn -= 1.0;
    }

    if turn != 0.0 {
        // turn_speed * dt 만큼 회전 각도를 만든다
        let angle = turn * worm.turn_speed * dt;

        let rot = Rot2::radians(angle);

        let rotated_vec = rot * worm.target_dir.as_vec2();

        worm.target_dir = Dir2::new(rotated_vec).unwrap();
    }

    // 2) 실제 방향(dir)은 target_dir을 "부드럽게 따라가게" 한다 (핵심: slerp)
    //    t가 0이면 거의 안 움직이고, t가 1이면 즉시 목표로 "확" 바뀜
    let t = (worm.turn_follow * dt).clamp(0.0, 1.0);
    worm.dir = worm.dir.slerp(worm.target_dir, t);
}

/// 머리를 시간 기반으로 이동시키고, 일정 거리마다 points에 기록
fn move_head(time: Res<Time>, mut worm: ResMut<Worm>) {
    let dt = time.delta_secs();
    // Dir2는 길이가 1인 "방향"이므로, as_vec2()로 Vec2를 꺼내서 위치 계산에 사용
    let new_head = worm.head + worm.dir.as_vec2() * worm.speed * dt;

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

fn check_collision(
    mut commands: Commands,
    mut worm: ResMut<Worm>,
    mut dots: ResMut<Dots>,
) {
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
            dots.spawn(&mut commands);
        }
    }
}