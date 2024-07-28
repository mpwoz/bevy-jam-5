use std::f32::consts;

use bevy::prelude::*;
use bevy::utils::HashSet;

use crate::game::grid::grid_layout::GridLayout;
use crate::game::grid::GridPosition;
use crate::game::line_of_sight::FacingWallsCache;
use crate::game::threat::{ThreatTimer, ThreatTimerSettings};
use crate::geometry_2d::line_segment::LineSegment;
use crate::AppSet;

pub fn plugin(app: &mut App) {
    // systems
    app.add_systems(
        Update,
        update_visible_squares.in_set(AppSet::Update), //.before(crate::game::spawn::enemy::follow_player),
    );

    // reflect
    app.register_type::<Facing>();
    app.register_type::<VisionAbility>();
}

#[derive(Bundle, Default, Clone)]
pub struct VisionBundle {
    pub facing: Facing,
    pub vision_ability: VisionAbility,
    pub visible_squares: VisibleSquares,
    pub facing_walls_cache: FacingWallsCache,
}

/// Which direction an enemy is looking
#[derive(Component, Reflect, Debug, Copy, Clone)]
#[reflect(Component)]
pub struct Facing(pub Vec2);
impl Default for Facing {
    fn default() -> Self {
        Self(Vec2::new(1., 0.))
    }
}

#[derive(Component, Reflect, Debug, Copy, Clone)]
#[reflect(Component)]
pub struct VisionAbility {
    pub field_of_view_radians: f32, // angle of cone of vision (total)
    pub range_in_grid_units: f32,
}

impl Default for VisionAbility {
    fn default() -> Self {
        Self::of(VisionArchetype::default())
    }
}

impl VisionAbility {
    pub fn of(archetype: VisionArchetype) -> Self {
        match archetype {
            VisionArchetype::Sniper => VisionAbility {
                field_of_view_radians: consts::FRAC_PI_8,
                range_in_grid_units: 10.0,
            },
            VisionArchetype::Patrol => VisionAbility {
                field_of_view_radians: consts::FRAC_PI_4,
                range_in_grid_units: 5.0,
            },
            VisionArchetype::Ghost => VisionAbility {
                field_of_view_radians: 2. * consts::PI,
                range_in_grid_units: 30.0,
            },
            VisionArchetype::Player => VisionAbility {
                field_of_view_radians: 2. * consts::PI,
                range_in_grid_units: 30.0,
            },
        }
    }
}

// maybe we can figure out a way to encode these in LDTK for easy enemy design
#[derive(Default)]
pub enum VisionArchetype {
    /// Very narrow FOV, Long range, short detection time
    Sniper,

    /// Medium FOV, Medium Range, Medium detection time
    Patrol,

    /// Like the player but less range
    Ghost,

    /// This is the player's FOV
    #[default]
    Player,
}

#[derive(Component, Reflect, Debug, Clone, Default)]
#[reflect(Component)]
pub struct VisibleSquares {
    pub visible_squares: HashSet<IVec2>,
    for_position: GridPosition,
}

impl VisibleSquares {
    pub fn contains(&self, target: &GridPosition) -> bool {
        self.visible_squares.contains(&IVec2::new(
            target.coordinates.x as i32,
            target.coordinates.y as i32,
        ))
    }
}

pub fn update_visible_squares(
    mut query: Query<(
        &GridPosition,
        &VisionAbility,
        &Facing,
        &FacingWallsCache,
        &mut VisibleSquares,
    )>,
    threat_timer: Res<ThreatTimer>,
    threat_settings: Res<ThreatTimerSettings>,
    grid: Res<GridLayout>,
) {
    for (grid_position, vision, facing, walls, mut visible_squares) in query.iter_mut() {
        // don't recompute if grid coordinates haven't changed and if not immediately after threat level has changed TODO change this to work off a next threat level trigger
        let ray_start = grid_position.coordinates;
        let vision_range_threat_adjusted =
            if threat_timer.current_level < threat_settings.levels - 1 {
                vision.range_in_grid_units
            } else {
                vision.range_in_grid_units * threat_settings.levels as f32
            };
        if ray_start.distance(visible_squares.for_position.coordinates) < 1.0
            && threat_timer.timer.duration().as_secs_f32()
                > threat_settings.seconds_between_levels
                    - threat_settings.seconds_between_levels / 10.0
        {
            continue;
        }
        //visible_squares.for_position = *grid_position;

        let mut new_squares = vec![];

        // iterate all squares in range and determine if they're visible or not
        let bb = grid.bounding_box(grid_position, vision_range_threat_adjusted);
        for ray_end in bb.coords_range() {
            // info!("{:?} -> {:?}", ray_start, ray_end);

            // too near
            if ray_end.distance(ray_start) <= 1.0 {
                if grid_position.coordinates + facing.0 == ray_end {
                    new_squares.push(ray_end);
                }
                continue;
            }

            // too far
            if ray_start.distance(ray_end) > vision_range_threat_adjusted {
                continue;
            }

            // angle too wide
            let ray = LineSegment::new(
                grid.grid_to_world(&GridPosition::new(ray_start.x, ray_start.y)),
                grid.grid_to_world(&GridPosition::new(ray_end.x, ray_end.y)),
            );
            // let ray = LineSegment::new(ray_start, ray_end);
            let angle = ray.segment2d.direction.angle_between(facing.0);
            if angle.abs() > vision.field_of_view_radians {
                continue;
            }

            // wall in the way
            let wall_in_the_way = walls.facing_wall_edges.iter().any(|w| ray.do_intersect(w));
            if wall_in_the_way {
                continue;
            }

            // we made it! the square is visible
            new_squares.push(ray_end);
        }

        visible_squares.visible_squares = HashSet::from_iter(
            new_squares
                .into_iter()
                .map(|v| IVec2::new(v.x as i32, v.y as i32)),
        );
    }
}
