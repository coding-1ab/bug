use bevy::{color::palettes::css::*, prelude::*};
use bevy_prototype_lyon::prelude::*;
use std::collections::VecDeque;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, ShapePlugin))
        .insert_resource(Worm::new())
        .add_systems(Startup, setup)
        .add_systems(Update, (input_dir, move_head, redraw_worm))
        .run();
}

#[derive(Resource)]
struct Worm {
    head: Vec2,
    dir: Vec2,                 // 이동 방향 (정규화)
    speed: f32,                // px/s
    points: VecDeque<Vec2>,    // 머리 위치 히스토리 = 몸통
    max_points: usize,         // 몸 길이 (샘플 수)
    sample_distance: f32,      // 이 거리 이상 이동해야 points에 추가
}

impl Worm {
    fn new() -> Self {
        let head = Vec2::new(-200.0, 0.0);
        let mut points = VecDeque::new();
        points.push_back(head);

        Self {
            head,
            dir: Vec2::new(1.0, 0.0),
            speed: 220.0,
            points,
            max_points: 120,
            sample_distance: 6.0,
        }
    }
}

#[derive(Component)]
struct WormShape;

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    // 초기 더미 path
    let path = ShapePath::new()
        .move_to(Vec2::new(-200.0, 0.0))
        .line_to(Vec2::new(-199.0, 0.0));

    commands
        .spawn(ShapeBuilder::with(&path).stroke((GREEN, 10.0)).build())
        .insert(WormShape);
}

/// 방향 전환(키 입력). (WASD / 화살표)
fn input_dir(keys: Res<ButtonInput<KeyCode>>, mut worm: ResMut<Worm>) {
    let mut d = Vec2::ZERO;

    if keys.pressed(KeyCode::ArrowUp) || keys.pressed(KeyCode::KeyW) {
        d.y += 1.0;
    }
    if keys.pressed(KeyCode::ArrowDown) || keys.pressed(KeyCode::KeyS) {
        d.y -= 1.0;
    }
    if keys.pressed(KeyCode::ArrowLeft) || keys.pressed(KeyCode::KeyA) {
        d.x -= 1.0;
    }
    if keys.pressed(KeyCode::ArrowRight) || keys.pressed(KeyCode::KeyD) {
        d.x += 1.0;
    }

    if d.length_squared() > 0.0 {
        worm.dir = d.normalize();
    }
}

/// 머리를 시간 기반으로 이동시키고, 일정 거리마다 points에 기록
fn move_head(time: Res<Time>, mut worm: ResMut<Worm>) {
    let dt = time.delta_secs();
    let new_head = worm.head + worm.dir * worm.speed * dt;

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
fn redraw_worm(worm: Res<Worm>, mut q: Query<&mut Shape, With<WormShape>>) {
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
    if let Some(mut shape) = q.iter_mut().next() {
        *shape = ShapeBuilder::with(&path).stroke((GREEN, 10.0)).build();
    }
}