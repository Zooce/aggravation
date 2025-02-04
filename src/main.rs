use bevy::prelude::*;

mod buttons;
mod components;
mod computer_turn;
mod constants;
mod choose_color;
mod dice_roll;
mod end_turn;
mod human_turn;
mod main_menu;
mod next_player;
mod power;
mod process;
mod resources;
mod shared_systems;
mod turn_setup;
mod vexation;

use constants::*;
use main_menu::*;
use vexation::VexationPlugin;

fn main() {
    App::new()
        // resources
        .insert_resource(ClearColor(Color::srgb(0.4, 0.4, 0.4)))

        .add_systems(Startup, setup)

        // plugins
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Vexation".to_string(),
                resolution: (WINDOW_SIZE, WINDOW_SIZE).into(),
                resizable: false,
                ..default()
            }),
            ..default()
        }).set(ImagePlugin::default_nearest()))
        .add_plugins(MainMenuPlugin)
        .add_plugins(VexationPlugin)

        // go!
        .run();
}

pub fn setup(mut commands: Commands) {
    commands.spawn(Camera2d::default());
}

// TODO: consider using https://github.com/IyesGames/iyes_loopless to organize this turn-based game
