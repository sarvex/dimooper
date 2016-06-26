extern crate sdl2;
extern crate sdl2_sys;
extern crate portmidi as pm;

use pm::types::MidiEvent;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::render::Renderer;
use sdl2::rect::{Point, Rect};

mod looper;
mod updatable;
mod midi;
mod graphicsprimitives;

use midi::Note;
use looper::{Looper, State};
use updatable::Updatable;
use graphicsprimitives::CircleRenderer;

const EVENT_LOOP_SLEEP_TIMEOUT: u64 = 3;
const CONTROL_CHANNEL_NUMBER: u8 = 9;
const CONTROL_KEY_NUMBER: u8 = 51;

macro_rules! colors {
    ($($hex:expr),*) => {
        &[$(
            Color::RGB((($hex & 0xFF0000) >> 16) as u8,
                       (($hex & 0xFF00) >> 8) as u8,
                       ($hex & 0xFF) as u8)
        ),*]
    }
}

const CHANNEL_PALETTE: &'static [Color; 5] = colors![0xF15A5A, 0xF0C419, 0x4EBA6F, 0x2D95BF,
                                                     0x955BA5];

fn events_to_notes(replay_buffer: &[MidiEvent]) -> Vec<Note> {
    let mut note_tracker: [[Option<u32>; 128]; 16] = [[None; 128]; 16];
    let mut result = Vec::new();

    use midi::MessageType::*;

    for event in replay_buffer {
        let channel = midi::get_note_channel(&event.message);
        match (midi::get_message_type(&event.message), midi::get_note_key(&event.message)) {
            (NoteOn, key) => {
                match note_tracker[channel as usize][key as usize] {
                    Some(start_timestamp) => {
                        result.push(Note {
                            start_timestamp: start_timestamp,
                            end_timestamp: event.timestamp,
                            key: key,
                            channel: channel,
                        });
                        note_tracker[channel as usize][key as usize] = Some(event.timestamp);
                    }
                    None => note_tracker[channel as usize][key as usize] = Some(event.timestamp),
                }
            }
            (NoteOff, key) => {
                if let Some(start_timestamp) = note_tracker[channel as usize][key as usize] {
                    result.push(Note {
                        start_timestamp: start_timestamp,
                        end_timestamp: event.timestamp,
                        key: key,
                        channel: channel,
                    });
                    note_tracker[channel as usize][key as usize] = None;
                }
            }
            (Other, _) => (),
        }
    }

    result
}

fn render_note(note: &Note,
               replay_buffer: &[MidiEvent],
               renderer: &mut Renderer,
               window_width: u32,
               window_height: u32) {
    let row_height = window_height as f32 / 128.0;
    let n = replay_buffer.len();
    let dt = (replay_buffer[n - 1].timestamp - replay_buffer[0].timestamp) as f32;

    let color = CHANNEL_PALETTE[note.channel as usize % CHANNEL_PALETTE.len()];

    let t1 = (note.start_timestamp - replay_buffer[0].timestamp) as f32;
    let t2 = (note.end_timestamp - replay_buffer[0].timestamp) as f32;
    let x1 = (t1 / dt * (window_width as f32 - 10.0) + 5.0) as i32;
    let x2 = (t2 / dt * (window_width as f32 - 10.0) + 5.0) as i32;
    let y = (row_height * (127 - note.key) as f32) as i32;

    renderer.set_draw_color(color);
    renderer.fill_rect(Rect::new(x1, y, (x2 - x1 + 1) as u32, row_height as u32)).unwrap();
}

fn render_bar(time_cursor: u32,
              replay_buffer: &[MidiEvent],
              renderer: &mut Renderer,
              window_width: u32,
              window_height: u32) {
    let n = replay_buffer.len();
    let dt = (replay_buffer[n - 1].timestamp - replay_buffer[0].timestamp) as f32;
    let x = ((time_cursor as f32) / dt * (window_width as f32 - 10.0) + 5.0) as i32;
    renderer.set_draw_color(Color::RGB(255, 255, 255));
    renderer.draw_line(Point::from((x, 0)), Point::from((x, window_height as i32)))
        .unwrap();
}

fn render_looper(looper: &Looper, renderer: &mut Renderer, window_width: u32, window_height: u32) {
    if looper.replay_buffer.len() > 1 {
        let replay_buffer = &looper.replay_buffer;
        let notes = events_to_notes(replay_buffer);

        for note in notes {
            render_note(&note, replay_buffer, renderer, window_width, window_height);
        }

        render_bar(looper.time_cursor,
                   replay_buffer,
                   renderer,
                   window_width,
                   window_height);
    }

    let r = 15;
    let p = 25;
    let x = window_width as i32 - r - 2 * p;
    let y = r + p;
    renderer.set_draw_color(Color::RGB(255, 0, 0));

    if let State::Recording = looper.state {
        renderer.fill_circle(x, y, r);
    } else {
        renderer.draw_circle(x, y, r);
    }
}

fn print_devices(pm: &pm::PortMidi) {
    for dev in pm.devices().unwrap() {
        println!("{}", dev);
    }
}

fn main() {
    let context = pm::PortMidi::new().unwrap();

    let (input_id, output_id) = {
        let args: Vec<String> = std::env::args().collect();

        if args.len() < 2 {
            print_devices(&context);
            println!("Usage: ./midi-looper <input-port> <output-port>");
            std::process::exit(1);
        }

        (args[1].trim().parse().unwrap(), args[2].trim().parse().unwrap())
    };

    let in_info = context.device(input_id).unwrap();
    println!("Listening on: {} {}", in_info.id(), in_info.name());
    let in_port = context.input_port(in_info, 1024).unwrap();

    let out_info = context.device(output_id).unwrap();
    println!("Sending recorded events: {} {}",
             out_info.id(),
             out_info.name());
    let mut out_port = context.output_port(out_info, 1024).unwrap();

    let window_width = 800;
    let window_height = 600;
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let mut timer_subsystem = sdl_context.timer().unwrap();

    let window = video_subsystem.window("Midi Looper", window_width, window_height)
        .position_centered()
        .opengl()
        .build()
        .unwrap();

    let mut renderer = window.renderer().build().unwrap();
    let mut event_pump = sdl_context.event_pump().unwrap();

    let mut looper = looper::Looper::new(&mut out_port);
    let mut running = true;

    let mut previuos_ticks = timer_subsystem.ticks();

    while running {
        let current_ticks = timer_subsystem.ticks();
        let delta_time = current_ticks - previuos_ticks;
        previuos_ticks = current_ticks;

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } |
                Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    running = false;
                }

                Event::KeyDown { keycode: Some(Keycode::Space), .. } => {
                    looper.toggle_recording();
                }

                Event::KeyDown { keycode: Some(Keycode::Z), .. } => {
                    looper.reset();
                }

                Event::KeyDown { keycode: Some(Keycode::Q), .. } => {
                    looper.toggle_pause();
                }

                _ => {}
            }
        }

        if let Ok(Some(events)) = in_port.read_n(1024) {
            for event in events {
                if midi::is_note_message(&event.message) &&
                   midi::get_note_channel(&event.message) == CONTROL_CHANNEL_NUMBER {
                    if midi::get_message_type(&event.message) == midi::MessageType::NoteOn &&
                       midi::get_note_key(&event.message) == CONTROL_KEY_NUMBER {
                        looper.toggle_recording();
                    }
                } else {
                    looper.on_midi_event(&event);
                }
            }
        }

        looper.update(delta_time);
        renderer.set_draw_color(Color::RGB(0, 0, 0));
        renderer.clear();
        render_looper(&looper, &mut renderer, window_width, window_height);
        renderer.present();

        std::thread::sleep(std::time::Duration::from_millis(EVENT_LOOP_SLEEP_TIMEOUT));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_hello() {
        assert!(true);
    }
}
