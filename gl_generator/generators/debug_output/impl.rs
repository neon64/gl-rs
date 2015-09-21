
//
// Rust implementation based upon the article here:
// http://renderingpipeline.com/2013/09/simulating-khr_debug-on-macos-x/
//
//
// Use-cases:
//
// Use this as a fallback on systems that don't implement KHR_debug for debugging only,
// don't use this code in shipping release builds - it will slow down the application!
// Using a debug callback instead of lots of glGetError() calls should work fine,
// MessageControl, DebugGroups and DebugLabels are only implemented as fallbacks, in case
// you have to rely on those features, you want to reimplement them in a more efficient way.
//
// Wrong behavior:
//
// * Does not support multiple OpenGL contexts, all errors from all contexts are mixed.
//   All settings (including the debug callback) are set for all contexts.
//
// * glObjectLabel and glObjectPtrLabel do not check if the object to label exists and thus
//   will not generate a GL_INVALID_VALUE.
//
// * glObjectLabel can label GL_DISPLAY_LIST even in Core profiles.
//
// Inefficiency:
//
// * Using this, the number of GL calls doubles as each call will get followed by a glGetError.
// * This will also force OpenGL to run synchronous which will reduce the performance!
// * ObjectLabels are implemented inefficiently and are not used internally. The functionality is
//   only present to be compatible with KHR_debug.
// * DebugGroups and glDebugMessageControl are not efficiently implemented.
//
// This implementation always behaves synchronous, even if GL_DEBUG_OUTPUT_SYNCHRONOUS is
// disabled (the default btw.). This is legal by the spec.
//

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
    if(!self.should_message_get_processed(source, ty, id, severity)) {
        return;
    }

    let mut state = self.debug_output.borrow_mut();

    match state.callback {
        Some(callback) => {
            callback(source, ty, id, severity, proper_length, buf, state.user_param)
        },
        None => {
            // no callback, store it in the log
            state.last_debug_message = Some(DebugMessage {
                source: source,
                ty: ty,
                id: id,
                severity: severity,
                length: length,
                buf: buf
            });
        }
    }
}

fn fallback_debug_message_control(&self, source: types::GLenum, ty: types::GLenum, severity: types::GLenum, count: types::GLsizei, ids: *const types::GLuint, enabled: types::GLboolean) {
    if(count != 0 && (source == DONT_CARE || ty == DONT_CARE || severity != DONT_CARE)) {
        // see KHR_debug 5.5.4
        self.insert_api_error(INVALID_OPERATION, "invalid operation in glDebugMessageControl: if an ID is specified, source and type have to be specified as well but severity has to be GL_DONT_CARE");
    }

    let ids = unsafe { __gl_imports::slice::from_raw_parts(ids, count as usize).to_vec() };

    let mut state = self.debug_output.borrow_mut();
    let debug_group = state.debug_group_number;

    state.rules.push(DebugMessageControlRule {
        source: source,
        ty: ty,
        severity: severity,
        enabled: enabled,
        debug_group: debug_group,
        ids: ids
    });
}

fn fallback_get_debug_message_log(&self, count: types::GLuint, bufsize: types::GLsizei, sources: *mut types::GLenum, types: *mut types::GLenum, ids: *mut types::GLuint, severities: *mut types::GLenum, lengths: *mut types::GLsizei, message_log: *mut types::GLchar) -> types::GLuint {
    if bufsize < 0 && message_log != __gl_imports::null_mut() {
        self.insert_api_error(INVALID_VALUE , "invalid value in glGetDebugMessageLog: bufsize < 0 and messageLog != NULL" );
        return 0;
    }

    let mut state = self.debug_output.borrow_mut();

    if count == 0 {
        return 0;
    }

    match state.last_debug_message.take() {
        Some(ref message) => {
            if types != __gl_imports::null_mut() { let mut v = unsafe { __gl_imports::slice::from_raw_parts(types, count as usize)[0] }; v = message.ty; }
            if sources != __gl_imports::null_mut() { let mut v = unsafe { __gl_imports::slice::from_raw_parts(sources, count as usize)[0] }; v = message.source; }
            if ids != __gl_imports::null_mut() { let mut v = unsafe { __gl_imports::slice::from_raw_parts(ids, count as usize)[0] }; v = message.id; }
            if severities != __gl_imports::null_mut() { let mut v = unsafe { __gl_imports::slice::from_raw_parts(severities, count as usize)[0] }; v = message.severity; }
            if lengths != __gl_imports::null_mut() { let mut v = unsafe { __gl_imports::slice::from_raw_parts(lengths, count as usize)[0] }; v = message.length; }

            // length is without the 0-termination
            if bufsize <= message.length {
                // won't fit, don't return the error :-(
                // 6.1.15 of KHR_debug
                return 0;
            }

            unsafe { __gl_imports::libc::strncpy(message_log, message.buf, bufsize as u64); }
            let mut null = unsafe { __gl_imports::slice::from_raw_parts(message_log, count as usize)[(bufsize-1) as usize] };
            null = 0;

            1
        },
        None => { return 0; }
    }
}

fn should_message_get_processed(&self, source: types::GLenum, ty: types::GLenum, id: types::GLuint, severity: types::GLenum) -> bool {
    // check from the newest to the oldest rule,
    // first one to be applyable to this message defines if it gets processed:
    for rule in self.debug_output.borrow().rules.iter().rev() {
        if rule_applies(&rule, source, ty, id, severity) {
            return rule.enabled == 1;
        }
    }

    // no matching rule found, apply default behavior:
    if severity == DEBUG_SEVERITY_LOW {
        return false;
    }

    true
}

/// artificially creates a gl error
fn insert_api_error(&self, ty: types::GLenum, message: &str) {
    self.debug_output.borrow_mut().last_error = ty;
    self.debug_message_insert_internal(DEBUG_SOURCE_API, DEBUG_TYPE_ERROR, ty, DEBUG_SEVERITY_HIGH, message.len() as i32, message.as_bytes().as_ptr() as *const i8);
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