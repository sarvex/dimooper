use pm::types::MidiMessage;

pub fn get_message_type(message: &MidiMessage) -> u8 {
    message.status & 0b11110000
}

pub fn is_note_message(message: &MidiMessage) -> bool {
    let message_type = get_message_type(message);
    message_type == 0b10000000 || message_type == 0b10010000
}
