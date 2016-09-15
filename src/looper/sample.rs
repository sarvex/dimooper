use sdl2::render::Renderer;

use midi;
use midi::{AbsMidiEvent, TypedMidiMessage, Note, MidiSink};
use measure::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct QuantMidiEvent {
    pub message: TypedMidiMessage,
    pub quant: Quant,
}

pub struct Sample {
    // FIXME(#153): Improve performance of the event look up in sample
    pub buffer: Vec<QuantMidiEvent>,
    pub amount_of_measures: u32,
    quant_shift: Quant,
    notes: Vec<Note>,
    sample_quant_length: Quant,
    quants_per_measure: Quant,
}

impl Sample {
    fn amount_of_measures_in_buffer(buffer: &[AbsMidiEvent], measure: &Measure) -> u32 {
        let n = buffer.len();

        if n > 0 {
            (buffer[n - 1].timestamp - buffer[0].timestamp + measure.measure_size_millis()) / measure.measure_size_millis()
        } else {
            1
        }
    }

    pub fn new(buffer: &[AbsMidiEvent], measure: &Measure, measure_shift: u32) -> Sample {
        let amount_of_measures = Self::amount_of_measures_in_buffer(buffer, measure);

        let quant_buffer = {
            let mut result = Vec::new();

            for event in buffer {
                result.push(QuantMidiEvent {
                    message: event.message,
                    quant: measure.snap_timestamp_to_quant(event.timestamp),
                })
            }

            result
        };

        let notes = midi::events_to_notes(&quant_buffer);

        Sample {
            buffer: quant_buffer,
            amount_of_measures: amount_of_measures,
            notes: notes,
            quant_shift: measure.measures_to_quants(measure_shift),
            sample_quant_length: Quant(amount_of_measures) * measure.quants_per_measure(),
            quants_per_measure: measure.quants_per_measure(),
        }
    }

    pub fn replay_quant<Sink: MidiSink>(&self, current_quant: Quant, sink: &mut Sink) {
        // FIXME(#152): More robust modulo implementation for Sample::replay_quant
        let sample_quant = {
            let normalized_quant = current_quant % self.sample_quant_length;

            if self.quant_shift > normalized_quant {
                self.sample_quant_length - (self.quant_shift - normalized_quant)
            } else {
                normalized_quant - self.quant_shift
            }
        };

        // FIXME(#153): Improve performance of the event look up in sample
        for event in &self.buffer {
            if event.quant == sample_quant {
                // FIXME(#141): Handle result of the sink message feeding
                sink.feed(event.message).unwrap();
            }
        }
    }

    fn measure_notes(&self, measure_number: u32) -> Vec<Note> {
        let start: Quant = Quant(measure_number) * self.quants_per_measure;
        let end: Quant = Quant(measure_number + 1) * self.quants_per_measure;
        let mut result = Vec::new();

        for note in &self.notes {
            if (start <= note.start_quant && note.start_quant <= end) || (start <= note.end_quant && note.end_quant <= end) {
                result.push(*note);
            }
        }

        result
    }

    // FIXME(#154): Sample rendering doesn't take into account the quant shift
    pub fn render(&self, raw_measure_number: u32, renderer: &mut Renderer) {
        let current_measure_number = raw_measure_number % self.amount_of_measures;
        let current_measure_notes = self.measure_notes(current_measure_number);
        let note_shift = Quant(current_measure_number) * self.quants_per_measure;

        for note in &current_measure_notes {
            note.render(renderer, self.quants_per_measure, note_shift);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Sample;
    use config::*;

    use measure::Measure;
    use midi::{AbsMidiEvent, TypedMidiMessage};

    const DEFAULT_MEASURE: Measure = Measure {
        tempo_bpm: DEFAULT_TEMPO_BPM,
        measure_size_bpm: DEFAULT_MEASURE_SIZE_BPM,
        quantation_level: DEFAULT_QUANTATION_LEVEL,
    };

    macro_rules! test_sample_data {
        (
            $([$key: expr, $start: expr, $duration: expr]),*
        ) => {
            &[$(AbsMidiEvent {
                timestamp: $start,
                message: TypedMidiMessage::NoteOn {
                    channel: 0,
                    key: $key,
                    velocity: 0,
                }
            },

            AbsMidiEvent {
                timestamp: $start + $duration - 1,
                message: TypedMidiMessage::NoteOff {
                    channel: 0,
                    key: $key,
                    velocity: 0,
                }
            }),*]
        }
    }

    macro_rules! test_msg {
        (on => $key:expr) => {
            TypedMidiMessage::NoteOn {
                channel: 0,
                key: $key,
                velocity: 0,
            }
        };

        (off => $key:expr) => {
            TypedMidiMessage::NoteOff {
                channel: 0,
                key: $key,
                velocity: 0,
            }
        };
    }

    #[test]
    fn test_amount_of_measure_calculation() {
        let expected_amount_of_measures = 2;

        let buffer = test_sample_data! [
            [0, 0, DEFAULT_MEASURE.measure_size_millis() * expected_amount_of_measures]
        ];

        // FIXME(#156): Add Unit Tests for shifted samples
        let sample = Sample::new(buffer, &DEFAULT_MEASURE, 0);

        println!("{}", sample.amount_of_measures);

        assert_eq!(expected_amount_of_measures, sample.amount_of_measures);
    }
}
