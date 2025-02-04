use bevy::prelude::*;
use crate::buttons::{ActionEvent, ButtonAction, ButtonSize, ButtonState};
use crate::components::{CurrentPlayer, Evading, Marble, Player, SelfJumping};
use crate::constants::{CENTER_INDEX, TILE_BUTTON_SIZE, TILE_SIZE, Z_UI};
use crate::resources::{CurrentPlayerData, DiceData, GameData, GameState, GameButtonAction, HumanPlayer};
use crate::shared_systems::SharedSystemSet;
use rand::thread_rng;
use rand::distributions::{ Distribution, WeightedIndex };

#[derive(Debug, Event)]
pub struct GeneratePowerUpEvent(pub Player);

#[derive(Debug, Event)]
pub enum PowerEvent {
    Capture{captor: Player, captive: Player},
    Index{player: Player, index: usize, prev_index: usize},
    Use{player: Player, index: usize},
}

#[derive(Event)]
pub enum PowerDownEvent {
    Evading(Player),
    SelfJumping(Player),
}

#[derive(Debug, Event)]
pub struct ActivatePowerUpEvent(pub PowerUp);

#[derive(Debug, Clone, Copy)]
pub enum PowerUp {
    RollAgain,       // weight = 4
    DoubleDice,      // weight = 4
    EvadeCapture,    // weight = 3
    SelfJump,        // weight = 2
    CaptureNearest,  // weight = 1
    HomeRun,         // weight = 1
}

const POWER_UP_WEIGHTS: [usize; 6] = [4, 4, 3, 2, 1, 1];

impl From<usize> for PowerUp {
    fn from(value: usize) -> Self {
        match value {
            0 => PowerUp::RollAgain,
            1 => PowerUp::DoubleDice,
            2 => PowerUp::EvadeCapture,
            3 => PowerUp::SelfJump,
            4 => PowerUp::CaptureNearest,
            5 => PowerUp::HomeRun,
            _ => unreachable!(),
        }
    }
}

#[derive(Resource)]
struct PowerUpDistribution(pub WeightedIndex<usize>);

#[derive(Resource)]
pub struct PowerUpSpriteImages {
    pub roll_again: Handle<Image>,
    pub double_dice: Handle<Image>,
    pub evade_capture: Handle<Image>,
    pub self_jump: Handle<Image>,
    pub capture_nearest: Handle<Image>,
    pub home_run: Handle<Image>,
}

#[derive(Resource)]
pub struct PowerUpHighlightImages {
    pub evading: Handle<Image>,
    pub self_jumping: Handle<Image>,
}

pub struct PowerUpPlugin;

impl Plugin for PowerUpPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_event::<ActivatePowerUpEvent>()
            .add_event::<GeneratePowerUpEvent>()
            .add_event::<PowerEvent>()
            .add_event::<PowerBarEvent>()
            .add_event::<PowerDownEvent>()

            .insert_resource(PowerUpDistribution(WeightedIndex::new(&POWER_UP_WEIGHTS).unwrap()))

            .add_systems(Update, (handle_power_events, generate_power_up, activate_power_up, power_down_event_handler)
                .in_set(SharedSystemSet)
            )
            ;
    }
}

#[derive(Component, Debug)]
pub struct PowerBar {
    pub power: f32,
    pub power_up_count: usize,
    pub origin: f32,
}

impl PowerBar {
    pub fn new(origin: f32) -> Self {
        Self {
            power: 0.,
            power_up_count: 0,
            origin,
        }
    }
}

pub const MAX_POWER: f32 = 10.0;
pub const MAX_POWER_UPS: usize = 3;

impl PowerBar {
    /// Update the power bar and return `true` if it's full.
    pub fn update(&mut self, delta: f32) -> bool {
        let new_power = (self.power + delta).max(0.0); // this reads really weird but it means this -> max(self.power + delta, 0.0)
        if new_power >= MAX_POWER {
            match self.power_up_count {
                0 | 1 => {
                    self.power = (new_power - MAX_POWER).max(0.0); // carry over
                    self.power_up_count += 1;
                    true
                }
                2 => {
                    self.power = 0.0; // reset
                    self.power_up_count += 1;
                    true
                }
                _ => false,
            }
        } else {
            if self.power_up_count < MAX_POWER_UPS {
                self.power = new_power;
            }
            false
        }
    }
}

#[derive(Event)]
pub struct PowerBarEvent {
    pub power: f32,
    pub player: Player,
}

fn handle_power_events(
    mut commands: Commands,
    mut game_data: ResMut<GameData>,
    mut power_events: EventReader<PowerEvent>,
    mut power_up_events: EventWriter<GeneratePowerUpEvent>,
    mut activate_events: EventWriter<ActivatePowerUpEvent>,
    mut power_bars: Query<(&mut PowerBar, &mut Transform, &Player)>,
) {
    for event in power_events.read() {
        for (player, power) in match event {
            PowerEvent::Capture{ captor, captive } => {
                vec![
                    (captor, Some(3.)),
                    (captive, Some(-3.)),
                ]
            },
            PowerEvent::Index{ player, index, prev_index } => {
                let distance = if *index == CENTER_INDEX {
                    // TODO: with the double dice power up, the longest move you can make is 24 spaces
                    // base (54)  -> center (53) = 7
                    // prev_index -> center (53) = (5 or 17 or 29) - prev_index + 1
                    match *prev_index {
                        54 => 7,
                        _ if (0..=5).contains(prev_index) => 5 - prev_index + 1,
                        _ if (6..=17).contains(prev_index) => 17 - prev_index + 1,
                        _ if (18..=29).contains(prev_index) => 29 - prev_index + 1,
                        _ => unreachable!(),
                    }
                } else {
                    // base (54)   -> index = index + 1
                    // center (53) -> index = index + 1 - 41
                    // prev_index  -> index = index - prev_index
                    match *prev_index {
                        54 => index + 1,
                        CENTER_INDEX => index + 1 - 41,
                        _ => index - prev_index,
                    }
                } as f32;
                let points = match index {
                    0..=47 => 1.0,
                    _ => 2.0,
                } * 10.0 * distance / 48.0;
                vec![(player, Some(points))]
            }
            PowerEvent::Use{ player, index } => {
                let (power_up, power_up_button) = game_data.players.get_mut(&player).unwrap().use_power_up(*index).unwrap();
                commands.entity(power_up_button).despawn();
                activate_events.send(ActivatePowerUpEvent(power_up));
                vec![(player, None)]
            }
        } {
            let (mut bar, mut transform, _) = power_bars.iter_mut().find(|(_, _, &p)| p == *player).unwrap();
            match power {
                Some(power) => {
                    let power_up = bar.update(power);
                    // power-fill sprite is 14 x 126 (that 126 represents 10 power points, so 126 / 10 = 12.6 pixels for every point)
                    transform.translation.y = bar.origin + bar.power * 12.6;
                    if power_up {
                        power_up_events.send(GeneratePowerUpEvent(*player));
                    }
                }
                None => { bar.power_up_count -= 1; }
            }
        }
    }
}

fn generate_power_up(
    mut power_up_events: EventReader<GeneratePowerUpEvent>,
    mut game_data: ResMut<GameData>,
    power_up_dist: Res<PowerUpDistribution>,
    mut commands: Commands,
    power_up_sprite_images: Res<PowerUpSpriteImages>,
    human_player: Res<HumanPlayer>,
) {
    let mut rng = thread_rng();
    for GeneratePowerUpEvent(player) in power_up_events.read() {
        // spawn the power up button first
        let (x, y) = match player {
            Player::Red => (-6.5, 2.5),
            Player::Green => (6.5, 2.5),
            Player::Blue => (6.5, -5.5),
            Player::Yellow => (-6.5, -5.5),
        };
        // get the next unused power-up slot
        let i = match game_data.players.get(&player).unwrap().power_ups {
            [None, _, _] => 0,
            [_, None, _] => 1,
            [_, _, None] => 2,
            _ => unreachable!(),
        };

        // randomly generate the power up
        let power_up: PowerUp = power_up_dist.0.sample(&mut rng).into();

        let sprite_sheet = Sprite{
            image: match power_up {
                PowerUp::RollAgain => power_up_sprite_images.roll_again.clone(),
                PowerUp::DoubleDice => power_up_sprite_images.double_dice.clone(),
                PowerUp::EvadeCapture => power_up_sprite_images.evade_capture.clone(),
                PowerUp::SelfJump => power_up_sprite_images.self_jump.clone(),
                PowerUp::CaptureNearest => power_up_sprite_images.capture_nearest.clone(),
                PowerUp::HomeRun => power_up_sprite_images.home_run.clone(),
            },
            ..default()
        };
        let transform = Transform::from_xyz(x * TILE_SIZE, (y + 1.5 * (i as f32)) * TILE_SIZE, Z_UI);
        let action = ButtonAction(ActionEvent(match i {
            0 => GameButtonAction::PowerUpOne(*player),
            1 => GameButtonAction::PowerUpTwo(*player),
            2 => GameButtonAction::PowerUpThree(*player),
            _ => unreachable!(),
        }));

        let power_up_button = if human_player.color == *player {
            // only want to add button state and size if this is for the human player - we don't want them interacting with the computer players' buttons
            commands.spawn((
                sprite_sheet,
                transform,
                action,
                ButtonState::NotHovered,
                ButtonSize(TILE_BUTTON_SIZE)
            )).id()
        } else {
            commands.spawn((sprite_sheet, transform, action)).id()
        };
        game_data.players.get_mut(&player).unwrap().power_ups[i] = Some((power_up, power_up_button));
    }
}

fn activate_power_up(
    mut commands: Commands,
    mut events: EventReader<ActivatePowerUpEvent>,
    mut next_state: ResMut<NextState<GameState>>,
    mut game_data: ResMut<GameData>,
    mut dice_data: ResMut<DiceData>,
    current_player_data: Res<CurrentPlayerData>,
    mut marbles: Query<Entity, (With<Marble>, With<CurrentPlayer>)>,
    power_up_highlight_images: Res<PowerUpHighlightImages>,
) {
    let player_data = game_data.players.get_mut(&current_player_data.player).unwrap();
    for event in events.read() {
        if let Some(new_state) = match event.0 {
            PowerUp::RollAgain => Some(GameState::DiceRoll),
            PowerUp::DoubleDice => {
                dice_data.dice.multiplier = 2;
                Some(GameState::TurnSetup)
            }
            PowerUp::EvadeCapture => {
                if !player_data.power_up_status.evade_capture() {
                    for marble in marbles.iter_mut() {
                        commands.entity(marble).insert(Evading)
                        .with_children(|parent| {
                            parent.spawn((
                                Evading,
                                Sprite{
                                    image: power_up_highlight_images.evading.clone(),
                                    ..default()
                                },
                                Transform::from_xyz(0., 0., 1.),
                            ));
                        });
                    }
                }
                None
            }
            PowerUp::SelfJump => {
                if !player_data.power_up_status.jump_self() {
                    for marble in marbles.iter_mut() {
                        commands.entity(marble).insert(SelfJumping)
                        .with_children(|parent| {
                            parent.spawn((
                                SelfJumping,
                                Sprite{
                                    image: power_up_highlight_images.self_jumping.clone(),
                                    ..default()
                                },
                                Transform::from_xyz(0., 0., 1.),
                            ));
                        });
                    }
                }
                Some(GameState::TurnSetup)
            }
            PowerUp::CaptureNearest => {
                player_data.power_up_status.capture_nearest();
                Some(GameState::TurnSetup)
            }
            PowerUp::HomeRun => {
                player_data.power_up_status.home_run();
                Some(GameState::TurnSetup)
            }
        } {
            next_state.set(new_state);
        }
    }
}

fn power_down_event_handler(
    mut commands: Commands,
    mut power_down_events: EventReader<PowerDownEvent>,
    marbles: Query<(Entity, &Player), With<Marble>>,
    evading: Query<(Entity, &Parent), With<Evading>>,
    jumping: Query<(Entity, &Parent), With<SelfJumping>>,
) {
    for event in power_down_events.read() {
        match event {
            PowerDownEvent::Evading(player) => {
                for (highlight_entity, parent) in evading.iter() {
                    if let Ok((marble_entity, marble_player)) = marbles.get(parent.get()) {
                        if player == marble_player {
                            commands.entity(marble_entity).remove::<Evading>();
                            commands.entity(highlight_entity).remove_parent().despawn();
                        }
                    }
                }
            }
            PowerDownEvent::SelfJumping(player) => {
                for (highlight_entity, parent) in jumping.iter() {
                    if let Ok((marble_entity, marble_player)) = marbles.get(parent.get()) {
                        if player == marble_player {
                            commands.entity(marble_entity).remove::<SelfJumping>();
                            commands.entity(highlight_entity).remove_parent().despawn();
                        }
                    }
                }
            }
        }
    }
}
