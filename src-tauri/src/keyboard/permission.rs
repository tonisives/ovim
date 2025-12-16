use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;

// ApplicationServices framework binding for accessibility
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrustedWithOptions(options: core_foundation::dictionary::CFDictionaryRef) -> bool;
}

const K_AX_TRUSTED_CHECK_OPTION_PROMPT: &str = "AXTrustedCheckOptionPrompt";

/// Check if the app has accessibility/input monitoring permission
pub fn check_accessibility_permission() -> bool {
    unsafe { AXIsProcessTrustedWithOptions(std::ptr::null()) }
}

/// Request accessibility permission (shows system prompt)
pub fn request_accessibility_permission() -> bool {
    unsafe {
        let key = CFString::new(K_AX_TRUSTED_CHECK_OPTION_PROMPT);
        let value = CFBoolean::true_value();

        let options = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), value.as_CFType())]);

        AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef())
    }
}
