struct DebugOutputState {
    enabled: bool,
    last_error: types::GLenum,
    callback: Option<types::GLDEBUGPROC>,
    user_param: *mut __gl_imports::libc::c_void
}

/*struct DebugMessage {
    source: types::GLenum,
    ty: types::GLenum,
    id: types::GLuint,
    severity: types::GLenum,
    length: types::GLsizei,
    buf: *const types::GLchar
}*/

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