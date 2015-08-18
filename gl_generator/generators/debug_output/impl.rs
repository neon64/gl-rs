extern "system" fn fallback_get_error(&self, original: &extern "system" fn() -> types::GLenum) -> types::GLenum {
    // if there was an error, report it. if not report the last global error
    // which might got set by the automatic error checks
    let mut current_error = original();
    if current_error == NO_ERROR {
        current_error = self.debug_output.borrow().last_error;
    }
    self.debug_output.borrow_mut().last_error = NO_ERROR;
    return current_error;
}

extern "system" fn fallback_debug_message_callback(&self, _: &extern "system" fn(types::GLDEBUGPROC, *mut __gl_imports::libc::c_void), callback: types::GLDEBUGPROC, user_param: *mut __gl_imports::libc::c_void) {
    self.debug_output.borrow_mut().callback = Some(callback);
    self.debug_output.borrow_mut().user_param = user_param;
}

/// Inserts a debug message
extern "system" fn fallback_debug_message_insert(&self, _: &extern "system" fn(types::GLenum, types::GLenum, types::GLuint, types::GLenum, types::GLsizei, *const types::GLchar), source: types::GLenum, ty: types::GLenum, id: types::GLuint, severity: types::GLenum, length: types::GLsizei, buf: *const types::GLchar) {
    if !self.debug_output.borrow().enabled { return }

    // calls from the application are a bit more restricted in the types of errors they are allowed to generate:
    if (source != DEBUG_SOURCE_APPLICATION) && (source != DEBUG_SOURCE_THIRD_PARTY) {
        self.insert_api_error(INVALID_ENUM, "invalid enum in glDebugMessageInsert: source has to be GL_DEBUG_SOURCE_APPLICATION or GL_DEBUG_SOURCE_THIRD_PARTY");
        return;
    }

    self.debug_message_insert_internal(source, ty, id, severity, length, buf);
}

/// Inserts a debug message
/// This is designed to be used internally by the generator
/// and therefore allows more freedom with the `source` parameter.
fn debug_message_insert_internal(&self, source: types::GLenum, ty: types::GLenum, id: types::GLuint, severity: types::GLenum, length: types::GLsizei, buf: *const types::GLchar) {
    if !self.debug_output.borrow().enabled { return }

    if !is_valid_severity(severity) {
        self.insert_api_error(INVALID_ENUM, "invalid enum in glDebugMessageInsert: severity is invalid");
        return;
    }
    if !is_valid_type(ty) {
        self.insert_api_error(INVALID_ENUM, "invalid enum in glDebugMessageInsert: type is invalid");
        return;
    }
    if !is_valid_source(source) {
        self.insert_api_error(INVALID_ENUM, "invalid enum in glDebugMessageInsert: source is invalid");
        return;
    }

    // length can be -1 which means that buf is 0 terminated.
    // however, the messages created should always set length to the number of chars in the message (excluding the trailing 0)
    let proper_length = if length < 0 { unsafe { __gl_imports::libc::strlen(buf) as i32 } } else { length };

    if proper_length > KHR_DEBUG_EMULATOR_MAX_DEBUG_MESSAGE_LENGTH {
        self.insert_api_error(INVALID_VALUE , "invalid value in glDebugMessageInsert: message is too long");
        return;
    }

    // there might be rules inserted by glDebugMessageControl to mute this message:
    /*if !should_message_get_processed(source, ty, id, severity) {
        return;
    }*/

    let state = self.debug_output.borrow();

    match state.callback {
        Some(callback) => {
            callback(source, ty, id, severity, proper_length, buf, state.user_param)
        },
        None => {
            // no callback, store it in the log
            /*g_LastDebugMessageEmpty = false;
            g_LastDebugMessage.source = source;
            g_LastDebugMessage.type   = type;
            g_LastDebugMessage.id     = id;
            g_LastDebugMessage.severity = severity;
            g_LastDebugMessage.length = length;
            g_LastDebugMessage.buf    = buf;*/
        }
    }
}

/// artificially creates a gl error
fn insert_api_error(&self, ty: types::GLenum, message: &str) {
    self.debug_output.borrow_mut().last_error = ty;
    /*println!("{:?}", message as *const _ as *const __gl_imports::libc::c_void);
    let message = __gl_imports::CString::new(message).unwrap();
    println!("{:?}", message);
    println!("{:?}", message.as_ptr());*/
    self.debug_message_insert_internal(DEBUG_SOURCE_API, DEBUG_TYPE_ERROR, ty, DEBUG_SEVERITY_HIGH, /*-1*/ message.len() as i32, message.as_bytes().as_ptr() as *const i8);
}

/// checks for an OpenGL error and reports it
fn check_error(&self, name: &str) {
    let check = unsafe { __gl_imports::mem::transmute::<_, extern "system" fn() -> types::GLenum>(self.GetError.get_original()) };
    let current_error = check();
    if current_error != NO_ERROR {
        self.insert_api_error(current_error, &get_error_string(current_error, name))
    }
}

/// Called after each call to an OpenGL function
pub fn on_fn_called(&self, name: &str) {
    self.check_error(name);
}