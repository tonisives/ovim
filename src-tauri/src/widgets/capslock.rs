use core_graphics::event::CGEventFlags;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventSourceFlagsState(stateID: i32) -> u64;
}

const COMBINED_SESSION_STATE: i32 = 0;

/// Check if Caps Lock is currently on
pub fn is_caps_lock_on() -> bool {
    unsafe {
        let flags = CGEventSourceFlagsState(COMBINED_SESSION_STATE);
        // CGEventFlags::CGEventFlagAlphaShift = 0x00010000
        (flags & CGEventFlags::CGEventFlagAlphaShift.bits()) != 0
    }
}
