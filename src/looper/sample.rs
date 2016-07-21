use midi::{TypedMidiEvent, TypedMidiMessage};
use measure::{Measure, Quant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct QuantMidiEvent {
    pub message: TypedMidiMessage,
    pub quant: Quant,
}

pub struct Sample {
    pub buffer: Vec<QuantMidiEvent>,
    pub amount_of_measures: u32,
    time_cursor: u32,
    measure: Measure,
}

impl Sample {
    fn amount_of_measures_in_buffer(buffer: &[TypedMidiEvent], measure: &Measure) -> u32 {
        let n = buffer.len();

        if n > 0 {
            (buffer[n - 1].timestamp - buffer[0].timestamp + measure.measure_size_millis()) / measure.measure_size_millis()
        } else {
            1
        }
    }

    pub fn new(buffer: &[TypedMidiEvent], measure: &Measure) -> Sample {
        let amount_of_measures = Self::amount_of_measures_in_buffer(&buffer, &measure);

        let quant_buffer = {
            let mut result = Vec::new();

            for event in buffer {
                result.push(QuantMidiEvent {
                    message: event.message,
                    quant: measure.timestamp_to_quant(event.timestamp),
                })
            }

            result
        };

        Sample {
            buffer: quant_buffer,
            amount_of_measures: amount_of_measures,
            time_cursor: amount_of_measures * measure.measure_size_millis(),
            measure: measure.clone(),
        }
    }

    pub fn update_measure(&mut self, new_measure: &Measure) {
        self.time_cursor =
            self.measure.scale_time_cursor(new_measure,
                                           self.amount_of_measures,
                                           self.time_cursor);

        self.measure = new_measure.clone();
    }

    pub fn get_next_messages(&mut self, delta_time: u32) -> Vec<TypedMidiMessage> {
        let next_time_cursor = self.time_cursor + delta_time;
        let sample_size_millis = self.measure.measure_size_millis() * self.amount_of_measures;
        let mut result = Vec::new();

        self.gather_messages_in_timerange(&mut result, self.time_cursor + 1, next_time_cursor);
        self.time_cursor = next_time_cursor % sample_size_millis;

        if next_time_cursor >= sample_size_millis {
            self.gather_messages_in_timerange(&mut result, 0, self.time_cursor);
        }

        result
    }

    fn gather_messages_in_timerange(&self, result: &mut Vec<TypedMidiMessage>, start: u32, end: u32) {
        for event in self.buffer.iter() {
            let timestamp = self.measure.quant_to_timestamp(event.quant);
            if start <= timestamp && timestamp <= end {
                result.push(event.message);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Sample;
    use config::*;

    use measure::Measure;
    use midi::{TypedMidiEvent, TypedMidiMessage};

    const DEFAULT_MEASURE: Measure = Measure {
        tempo_bpm: DEFAULT_TEMPO_BPM,
        measure_size_bpm: DEFAULT_MEASURE_SIZE_BPM,
        quantation_level: DEFAULT_QUANTATION_LEVEL,
    };

    macro_rules! test_sample_data {
        (
            $([$key: expr, $start: expr, $duration: expr]),*
        ) => {
            &[$(TypedMidiEvent {
                timestamp: $start,
                message: TypedMidiMessage::NoteOn {
                    channel: 0,
                    key: $key,
                    velocity: 0,
                }
            },

            TypedMidiEvent {
                timestamp: $start + $duration - 1,
                message: TypedMidiMessage::NoteOff {
                    channel: 0,
                    key: $key,
                    velocity: 0,
                }
            }),*]
        }
    }

    #[test]
    fn test_get_next_messages() {
        let buffer = test_sample_data! [
            [1,
             0,
             DEFAULT_MEASURE.measure_size_millis()],
            [2,
             DEFAULT_MEASURE.measure_size_millis() + DEFAULT_MEASURE.quant_size_millis(),
             DEFAULT_MEASURE.measure_size_millis() - DEFAULT_MEASURE.quant_size_millis()]
        ];

        let mut sample = Sample::new(buffer, &DEFAULT_MEASURE);
        assert_eq!(2, sample.amount_of_measures);

        { // First Iteration
            let message = sample.get_next_messages(DEFAULT_MEASURE.measure_size_millis());
            assert_eq!(vec![
                TypedMidiMessage::NoteOn {
                    channel: 0,
                    key: 1,
                    velocity: 0,
                },

                TypedMidiMessage::NoteOff {
                    channel: 0,
                    key: 1,
                    velocity: 0,
                },
            ], message);
        }

        { // Second Iteration
            let message = sample.get_next_messages(DEFAULT_MEASURE.measure_size_millis() / 2);
            assert_eq!(vec![
                TypedMidiMessage::NoteOn {
                    channel: 0,
                    key: 2,
                    velocity: 0,
                },
            ], message);
        }

        { // Third Iteration
            let message = sample.get_next_messages(DEFAULT_MEASURE.measure_size_millis());
            assert_eq!(vec![
                TypedMidiMessage::NoteOff {
                    channel: 0,
                    key: 2,
                    velocity: 0,
                },

                TypedMidiMessage::NoteOn {
                    channel: 0,
                    key: 1,
                    velocity: 0,
                },
            ], message);
        }
    }

    #[test]
    fn test_amount_of_measure_calculation() {
        let expected_amount_of_measures = 2;

        let buffer = test_sample_data! [
            [0, 0, DEFAULT_MEASURE.measure_size_millis() * expected_amount_of_measures]
        ];

        let sample = Sample::new(buffer, &DEFAULT_MEASURE);

        println!("{}", sample.amount_of_measures);

        assert_eq!(expected_amount_of_measures, sample.amount_of_measures);
    }
}
