// SPDX-License-Identifier: MIT

//! Microphone permission, where the platform has one.
//!
//! Two functions with the same names on every target: [`microphone_granted`]
//! and [`request_microphone`]. Desktop has no runtime permission model, so
//! there it answers "granted" and requesting is a no-op — the whole flow in
//! [`audio_input`](crate::audio_input) is then written once, unconditionally,
//! with no `#[cfg]` at the call site.
//!
//! Android is the target that actually needs it: `RECORD_AUDIO` is a
//! "dangerous" permission, so since API 23 it must be granted by the user at
//! runtime, and until it is, opening a cpal input stream fails. That failure
//! is indistinguishable from a broken device, which is why
//! [`MicStatus::AwaitingPermission`](crate::audio_input::MicStatus) exists
//! separately from `Failed`.

/// Whether the app may open the microphone right now.
///
/// Always `true` off Android: desktop grants access at the OS level (or the
/// stream simply fails), and there is no API to consult.
#[cfg(not(target_os = "android"))]
pub fn microphone_granted() -> bool {
    true
}

/// No-op off Android — nothing to ask for.
#[cfg(not(target_os = "android"))]
pub fn request_microphone() {}

/// `Activity.checkSelfPermission(RECORD_AUDIO) == PERMISSION_GRANTED`.
///
/// Any JNI error is reported as *not* granted: the honest answer when the
/// check itself couldn't run is "don't assume we may record".
#[cfg(target_os = "android")]
pub fn microphone_granted() -> bool {
    match with_activity(|env, activity| {
        let permission = env.new_string(RECORD_AUDIO)?;
        let result = env
            .call_method(
                activity,
                "checkSelfPermission",
                "(Ljava/lang/String;)I",
                &[(&permission).into()],
            )?
            .i()?;
        // android.content.pm.PackageManager.PERMISSION_GRANTED
        Ok(result == 0)
    }) {
        Ok(granted) => granted,
        Err(err) => {
            bevy::log::warn!("Could not check RECORD_AUDIO permission: {err}");
            false
        }
    }
}

/// Shows the system permission dialog, if it hasn't already been answered.
///
/// Fire-and-forget: the result arrives on the Java side, in an
/// `onRequestPermissionsResult` callback belonging to an activity this crate
/// doesn't own. Rather than route that back across JNI,
/// [`audio_input::retry_capture_when_permission_granted`](crate::audio_input::
/// retry_capture_when_permission_granted) polls [`microphone_granted`] — the
/// dialog is a once-per-install event, so a cheap poll while it's open costs
/// nothing worth optimising.
#[cfg(target_os = "android")]
pub fn request_microphone() {
    if let Err(err) = with_activity(|env, activity| {
        let permission = env.new_string(RECORD_AUDIO)?;
        let permissions =
            env.new_object_array(1, "java/lang/String", jni::objects::JObject::null())?;
        env.set_object_array_element(&permissions, 0, &permission)?;
        env.call_method(
            activity,
            "requestPermissions",
            "([Ljava/lang/String;I)V",
            &[(&permissions).into(), REQUEST_CODE.into()],
        )?;
        Ok(())
    }) {
        bevy::log::warn!("Could not request RECORD_AUDIO permission: {err}");
    }
}

#[cfg(target_os = "android")]
const RECORD_AUDIO: &str = "android.permission.RECORD_AUDIO";

/// Passed to `requestPermissions` and handed back to the activity's
/// `onRequestPermissionsResult`. Nothing here reads it — see
/// [`request_microphone`] — but the API requires one.
#[cfg(target_os = "android")]
const REQUEST_CODE: i32 = 0;

/// Runs `f` with a JNI environment attached to the current thread and the
/// app's **`Activity`** object.
///
/// Deliberately *not* `ndk_context::android_context()`, which is the obvious
/// route and silently wrong here: `android-activity` registers the
/// **`Application`** with `ndk_context`, not the Activity (see its
/// `init.rs`, `initialize_android_context(vm, app_global)`). `Application`
/// is a `Context`, so `checkSelfPermission` resolves on it and appears to
/// work — but `requestPermissions` is declared on `Activity`, so it dies
/// with `NoSuchMethodError: no non-static method
/// "Landroid/app/Application;.requestPermissions..."` at runtime.
///
/// The Activity comes from `AndroidApp::activity_as_ptr`, which Bevy stores
/// in `ANDROID_APP` when `android_main` hands it over
/// (`crates/harmonicon-android`).
///
/// The returned reference is borrowed, not owned: per `activity_as_ptr`'s
/// own docs it must not be wrapped in a `Global` or `Auto`, either of which
/// would try to delete a reference we don't own. A bare `JObject` is inert
/// on drop, which is what makes this sound.
#[cfg(target_os = "android")]
fn with_activity<T>(
    f: impl FnOnce(&mut jni::JNIEnv, &jni::objects::JObject) -> jni::errors::Result<T>,
) -> jni::errors::Result<T> {
    let app = bevy::android::ANDROID_APP
        .get()
        .ok_or(jni::errors::Error::NullPtr("ANDROID_APP is not initialized"))?;
    // SAFETY: both pointers belong to the `AndroidApp` the platform handed to
    // `android_main`, and outlive the process's use of them.
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) }?;
    let activity = unsafe { jni::objects::JObject::from_raw(app.activity_as_ptr().cast()) };
    let mut env = vm.attach_current_thread()?;
    let result = f(&mut env, &activity);

    // A throwing JNI call leaves the exception *pending* on this thread, and
    // every later call then fails with the same opaque "Java exception was
    // thrown" — so a single failure poisons the whole polling loop. Describe
    // it (which dumps the real Throwable and its stack to logcat, the only
    // way to see what actually went wrong) and clear it before returning.
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
    }
    result
}
