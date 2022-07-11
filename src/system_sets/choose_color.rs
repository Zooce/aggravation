use bevy::prelude::*;
use bevy::input::mouse::MouseButton;
use bevy::window::CursorMoved;
use crate::components::*;
use crate::constants::*;
use crate::resources::*;

/// This system runs when we enter the ChooseColor state to clear out mouse
/// button clicks that carry over from the main menu.
pub fn clear_mouse_events(
    mut mouse_buttons: ResMut<Input<MouseButton>>,
) {
    mouse_buttons.clear();
}

pub fn mouse_hover_handler(
    commands: Commands,
    mut cursor_moved: EventReader<CursorMoved>,
    mut choose_color_data: ResMut<ChooseColorData>,
) {
    if let Some(event) = cursor_moved.iter().last() {
        let color = position_to_color(event.position);
        if color.is_some()
            && (choose_color_data.current_color.is_none()
                || choose_color_data.current_color != color)
        {
            choose_color_data.current_color = color;
            show_mask(commands, choose_color_data);
        }
    }
}

fn position_to_color(pos: Vec2) -> Option<Player> {
    let lr = if pos.x < WINDOW_SIZE / 2. {
        0
    } else {
        1
    };
    let bt = if pos.y < WINDOW_SIZE / 2. {
        0
    } else  {
        1
    };
    match (lr, bt) {
        (0, 0) => Some(Player::Yellow),
        (0, 1) => Some(Player::Red),
        (1, 0) => Some(Player::Blue),
        (1, 1) => Some(Player::Green),
        _ => None,
    }
}

pub fn mouse_click_handler(
    mut commands: Commands,
    mut state: ResMut<State<GameState>>,
    windows: Res<Windows>,
    mouse_buttons: Res<Input<MouseButton>>,
) {
    if mouse_buttons.just_pressed(MouseButton::Left) {
        let cursor = windows.get_primary().unwrap().cursor_position().unwrap();
        if let Some(color) = position_to_color(cursor) {
            commands.insert_resource(HumanPlayer{ color });
            state.set(GameState::NextPlayer).unwrap();
        }
    }
}

fn show_mask(mut commands: Commands, mut choose_color_data: ResMut<ChooseColorData>) {
    if let Some(mask) = choose_color_data.current_mask {
        commands.entity(mask).despawn();
    }
    choose_color_data.current_mask = Some(commands.spawn_bundle(SpriteBundle{
        texture: choose_color_data.masks[choose_color_data.current_color.unwrap() as usize].clone(),
        transform: Transform::from_xyz(0., 0., 3.),
        ..default()
    }).id());
}

pub fn human_player_chosen(
    mut commands: Commands,
    mut choose_color_data: ResMut<ChooseColorData>,
) {
    if let Some(mask) = choose_color_data.current_mask {
        commands.entity(mask).despawn();
        choose_color_data.current_mask = None;
    }
}
