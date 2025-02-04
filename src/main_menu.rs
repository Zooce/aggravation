use bevy::prelude::*;
use bevy::app::AppExit;
use bevy::window::PrimaryWindow;
use crate::buttons::*;
use crate::constants::*;
use crate::resources::*;

#[derive(Clone, Copy)]
enum MainMenuAction {
    StartGame,
    NextPage,
    PrevPage,
    Quit,
}

#[derive(Resource)]
struct RootUiEntities{
    pub ui: Vec<Entity>,
}

#[derive(Resource)]
struct UiAssets {
    // pub font: Handle<Font>,
    pub mini_font: Handle<Font>,
    pub title: Handle<Image>,
    pub play_button: Handle<TextureAtlas>,
    pub rules_button: Handle<TextureAtlas>,
    pub quit_button: Handle<TextureAtlas>,
    pub back_button: Handle<TextureAtlas>,
    pub next_button: Handle<TextureAtlas>,
}

#[derive(Resource)]
struct UiPageNumber(pub usize);

pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_state::<GameState>() // the starting state
            .add_event::<ActionEvent<MainMenuAction>>()

            .insert_resource(UiPageNumber(0))

            .add_systems(Startup, setup)

            .add_systems(OnEnter(GameState::MainMenu), main_menu_enter)
            // I'm executing button actions first because I want a frame
            // delay here so we can see the button animation
            .add_systems(Update, (
                    execute_menu_action,
                    mouse_watcher::<MainMenuAction>,
                    watch_button_state_changes,
                    menu_page_renderer
                ).chain()
                .run_if(in_state(GameState::MainMenu))
            )
            .add_systems(OnExit(GameState::MainMenu), main_menu_exit)
            ;
    }
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
) {
    let grid = (3, 1);
    commands.insert_resource(UiAssets{
        // font: asset_server.load("Kenney Thick.ttf"),
        mini_font: asset_server.load("Kenney Mini.ttf"),
        title: asset_server.load("title.png"),
        play_button: load_sprite_sheet("buttons/play_button.png", UI_BUTTON_SIZE, grid, &asset_server, &mut texture_atlases),
        rules_button: load_sprite_sheet("buttons/rules_button.png", UI_BUTTON_SIZE, grid, &asset_server, &mut texture_atlases),
        quit_button: load_sprite_sheet("buttons/quit_button.png", UI_BUTTON_SIZE, grid, &asset_server, &mut texture_atlases),
        back_button: load_sprite_sheet("buttons/back_button.png", UI_BUTTON_SIZE, grid, &asset_server, &mut texture_atlases),
        next_button: load_sprite_sheet("buttons/next_button.png", UI_BUTTON_SIZE, grid, &asset_server, &mut texture_atlases),
    });
}

fn main_menu_enter(
    mut commands: Commands,
    ui_assets: Res<UiAssets>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mouse_button_input: Res<Input<MouseButton>>,
) {
    let Ok(w) = windows.get_single() else {
        return;
    };
    let mouse_pressed = mouse_button_input.pressed(MouseButton::Left);
    let ui = create_main_menu(&mut commands, &ui_assets, w.cursor_position(), mouse_pressed);
    commands.insert_resource(RootUiEntities{ ui });
}

fn main_menu_exit(
    mut commands: Commands,
    root_entities: Res<RootUiEntities>,
) {
    for entity in &root_entities.ui {
        commands.entity(*entity).despawn_recursive();
    }
    commands.remove_resource::<RootUiEntities>();
}

fn execute_menu_action(
    mut action_events: EventReader<ActionEvent<MainMenuAction>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut page_number: ResMut<UiPageNumber>,
    mut app_exit_events: EventWriter<AppExit>,
) {
    for action in action_events.iter() {
        match action.0 {
            MainMenuAction::StartGame => next_state.set(GameState::GameStart),
            MainMenuAction::NextPage => page_number.0 += 1,
            MainMenuAction::PrevPage => page_number.0 -= 1,
            MainMenuAction::Quit => app_exit_events.send(AppExit),
        }
    }
}

/// Renders the current page in the menu if a page change occurred.
fn menu_page_renderer(
    page_number: Res<UiPageNumber>,
    mut current_page_number: Local<Option<usize>>,
    mut commands: Commands,
    mut root_entities: ResMut<RootUiEntities>,
    ui_assets: Res<UiAssets>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mouse_button_input: Res<Input<MouseButton>>,
) {
    // check to see if we event need to render anything
    let render_page = match *current_page_number {
        Some(p) if page_number.0 != p => {
            // destroy the current page so the next one can be rendered
            for entity in &root_entities.ui {
                commands.entity(*entity).despawn_recursive();
            }
            Some(page_number.0)
        }
        None => { // this is the first time this system has run - just set the local current page number
            *current_page_number = Some(page_number.0);
            None
        }
        _ => None,
    };

    let Ok(w) = windows.get_single() else {
        return;
    };
    let mouse_pressed = mouse_button_input.pressed(MouseButton::Left);
    if let Some(p) = render_page {
        *current_page_number = render_page;
        let ui = match p {
            0 => create_main_menu(&mut commands, &ui_assets, w.cursor_position(), mouse_pressed),
            1 | 2 | 3 => create_rules_page(&mut commands, ui_assets, page_number, w.cursor_position(), mouse_pressed),
            _ => unreachable!(),
        };
        root_entities.ui = ui;
    }
}

fn create_main_menu(
    commands: &mut Commands,
    ui_assets: &Res<UiAssets>,
    cursor_pos: Option<Vec2>,
    mouse_pressed: bool,
) -> Vec<Entity> {
    let root = commands
        .spawn(SpatialBundle::default())
        .with_children(|parent| {
            // title
            let y_title = 100.0;
            parent
                .spawn(SpriteBundle{
                    texture: ui_assets.title.clone(),
                    // this is 100 pixels upward from the center of the screen
                    transform: Transform::from_xyz(0.0, y_title, 1.0),
                    ..default()
                });

            // buttons
            let mut transform = Transform::from_xyz(0.0, y_title - 100.0, 1.0); // @ (0, 0) or the center of the screen
            spawn_sprite_sheet_button(
                parent,
                ui_assets.play_button.clone(),
                transform,
                ButtonAction(ActionEvent(MainMenuAction::StartGame)),
                Visibility::Inherited,
                get_button_state(cursor_pos, transform.translation, UI_BUTTON_SIZE.clone(), mouse_pressed),
                ButtonSize(UI_BUTTON_SIZE.clone()),
            );

            let y_offset = 48.0 + 20.0; // 48 = height of a button, 20 = spacing between buttons
            transform.translation -= Vec3::new(0.0, y_offset, 0.0);
            spawn_sprite_sheet_button(
                parent,
                ui_assets.rules_button.clone(),
                transform,
                ButtonAction(ActionEvent(MainMenuAction::NextPage)),
                Visibility::Inherited,
                get_button_state(cursor_pos, transform.translation, UI_BUTTON_SIZE.clone(), mouse_pressed),
                ButtonSize(UI_BUTTON_SIZE.clone()),
            );

            transform.translation -= Vec3::new(0.0, y_offset, 0.0);
            spawn_sprite_sheet_button(
                parent,
                ui_assets.quit_button.clone(),
                transform,
                ButtonAction(ActionEvent(MainMenuAction::Quit)),
                Visibility::Inherited,
                get_button_state(cursor_pos, transform.translation, UI_BUTTON_SIZE.clone(), mouse_pressed),
                ButtonSize(UI_BUTTON_SIZE.clone()),
            );
        })
        .id()
        ;

    vec![root]
}

const RULES_P1: &str =
r#"- Objective -

Move all your marbles counter-clockwise around the board from your BASE to your HOME row.

- Movement -

You can use either the value of a single die or the sum of the dice to move a marble. Once both dice values have been used to make moves, your turn is over.

NOTE - The dice are rolled automatically at the beginning of your turn.
"#;
const RULES_P2: &str =
r#"- Base -

You must roll a 1 or a 6 to exit the BASE.

- Jumping -

You may jump opponents' marbles but NOT your own.

- Capturing -

Landing on an opponent's marble captures it, sending it back to its BASE.
"#;
const RULES_P3: &str =
r#"- Center Tile -

The center tile is a special space allowing a marble to skip ahead on the board.

A marble can only enter using the exact sum of the dice.

A marble can only enter from a corner with a different colored arrow.

A marble can only exit with a die roll of 1 but can then optionally use the other die to continue thier move.

A marble can only exit to the corner with the same colored arrow.
"#;
// TOOD: POWER-UPS

fn create_rules_page(
    commands: &mut Commands,
    ui_assets: Res<UiAssets>,
    page_number: Res<UiPageNumber>,
    cursor_pos: Option<Vec2>,
    mouse_pressed: bool,
) -> Vec<Entity> {
    let text = commands
        .spawn(TextBundle{
            text: Text::from_section(
                match page_number.0 {
                    1 => RULES_P1,
                    2 => RULES_P2,
                    3 => RULES_P3,
                    _ => unreachable!(),
                },
                TextStyle{
                    font: ui_assets.mini_font.clone(),
                    font_size: 24.0,
                    color: Color::WHITE,
                }
            ),
            style: Style{
                width: Val::Px(WINDOW_SIZE - 10.0 * 2.0),
                height: Val::Auto,
                align_self: AlignSelf::FlexStart,
                left: Val::Px(10.0),
                ..default()
            },
            ..default()
        })
        .id()
        ;

    let buttons = commands
        .spawn(SpatialBundle::default())
        .with_children(|parent| {
            const BOTTOM_BUTTON_Y: f32 = (-WINDOW_SIZE / 2.0) + TILE_SIZE;
            let x_offset = match page_number.0 {
                1 | 2 => {
                    let x_offset = (160.0 / 2.0) + 20.0;
                    let transform = Transform::from_xyz(x_offset, BOTTOM_BUTTON_Y, 5.0);
                    spawn_sprite_sheet_button(
                        parent,
                        ui_assets.next_button.clone(),
                        transform,
                        ButtonAction(ActionEvent(MainMenuAction::NextPage)),
                        Visibility::Inherited,
                        get_button_state(cursor_pos, transform.translation, UI_BUTTON_SIZE.clone(), mouse_pressed),
                        ButtonSize(UI_BUTTON_SIZE.clone()),
                    );
                    Some(-x_offset)
                }
                _ => None,
            };
            let transform = Transform::from_xyz(x_offset.unwrap_or_default(), BOTTOM_BUTTON_Y, 5.0);
            spawn_sprite_sheet_button(
                parent,
                ui_assets.back_button.clone(),
                transform,
                ButtonAction(ActionEvent(MainMenuAction::PrevPage)),
                Visibility::Inherited,
                get_button_state(cursor_pos, transform.translation, UI_BUTTON_SIZE.clone(), mouse_pressed),
                ButtonSize(UI_BUTTON_SIZE.clone()),
            );
        })
        .id()
        ;

    vec![text, buttons]
}
