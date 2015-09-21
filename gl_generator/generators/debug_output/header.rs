struct DebugOutputState {
    enabled: bool,
    last_error: types::GLenum,
    last_debug_message: Option<DebugMessage>,
    debug_group_number: u32,
    rules: Vec<DebugMessageControlRule>,
    callback: Option<types::GLDEBUGPROC>,
    user_param: *mut __gl_imports::libc::c_void
}

struct DebugMessage {
    source: types::GLenum,
    ty: types::GLenum,
    id: types::GLuint,
    severity: types::GLenum,
    length: types::GLsizei,
    buf: *const types::GLchar
}

struct DebugMessageControlRule {
    source: types::GLenum,
    ty: types::GLenum,
    severity: types::GLenum,
    ids: Vec<types::GLuint>,
    enabled: types::GLboolean,
    debug_group: types::GLuint
}

/// Implementation dependent limits:
///
/// * GL_MAX_DEBUG_MESSAGE_LENGTH and Gl_MAX_LABEL_LENGTH are arbitrary and can be changed.
/// * GL_MAX_DEBUG_GROUP_STACK_DEPTH is set to the lowest allowed value of 64 but can be changed
/// * GL_DEBUG_LOGGED_MESSAGES is set to 1 - increasing this will be more work.

static KHR_DEBUG_EMULATOR_MAX_DEBUG_MESSAGE_LENGTH: i32 = 256;

fn is_valid_severity(severity: types::GLenum) -> bool {
    match severity {
        DEBUG_SEVERITY_HIGH | DEBUG_SEVERITY_MEDIUM | DEBUG_SEVERITY_LOW | DEBUG_SEVERITY_NOTIFICATION => true,
        _ => false
    }
}

fn is_valid_type(ty: types::GLenum) -> bool {
    match ty {
        DEBUG_TYPE_ERROR | DEBUG_TYPE_DEPRECATED_BEHAVIOR | DEBUG_TYPE_UNDEFINED_BEHAVIOR | DEBUG_TYPE_PERFORMANCE | DEBUG_TYPE_PORTABILITY | DEBUG_TYPE_OTHER | DEBUG_TYPE_MARKER | DEBUG_TYPE_PUSH_GROUP | DEBUG_TYPE_POP_GROUP => true,
        _ => false
    }
}

fn is_valid_source(source: types::GLenum) -> bool {
    match source {
        DEBUG_SOURCE_API | DEBUG_SOURCE_SHADER_COMPILER | DEBUG_SOURCE_WINDOW_SYSTEM | DEBUG_SOURCE_THIRD_PARTY | DEBUG_SOURCE_APPLICATION | DEBUG_SOURCE_OTHER => true,
        _ => false
    }
}

fn rule_applies(rule: &DebugMessageControlRule, source: types::GLenum, ty: types::GLenum, id: types::GLuint, severity: types::GLenum) -> bool {
    // if no ids match
    if !rule.ids.is_empty() && !rule.ids.iter().any(|rule_id| *rule_id == id) { return false; }
    if rule.source != DONT_CARE && rule.source != source { return false; } // source mismatch
    if rule.ty != DONT_CARE && rule.ty != ty { return false }; // type mismatch
    if rule.severity != DONT_CARE && rule.severity != severity { return false }; // severity mismatch

    return true;
}

fn get_error_string(error_code: types::GLenum, name: &str) -> String {
    let part = match error_code {
        INVALID_ENUM => "invalid enum",
        INVALID_VALUE => "invalid value",
        INVALID_OPERATION => "invalid operation",
        INVALID_FRAMEBUFFER_OPERATION => "invalid framebuffer operation",
        OUT_OF_MEMORY => "out of memory",
        NO_ERROR => "no error",
        /*STACK_UNDERFLOW => "stack underflow",
        STACK_OVERFLOW => "stack overflow",*/
        _ => "unknown error"
    };

    format!("{error} in {place}", error = part, place = name)
}