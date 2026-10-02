// SPDX-License-Identifier: MIT

//! `android_main`, the Android entry point.
//!
//! Android never calls a `main`: the activity loads this shared library and
//! calls `android_main`, handing over the `AndroidApp` that owns the event
//! loop and the JNI handles. Bevy reads that back out of a global
//! (`bevy::android::ANDROID_APP`) when its winit backend starts, so it has to
//! be stashed there *before* the app runs.
//!
//! This is `#[bevy_main]`'s expansion, written out. The macro insists on
//! being applied to a function literally named `main`, which would mean a
//! stray `pub fn main` in a library and a dead one on every other target;
//! spelling it out costs four lines and says plainly what happens.

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(android_app: bevy::android::android_activity::AndroidApp) {
    init_certificate_verifier(&android_app);
    let _ = bevy::android::ANDROID_APP.set(android_app);
    harmonicon::run();
}

/// Content packs download over https, and reqwest verifies certificates
/// through `rustls-platform-verifier`, which on Android asks the system trust
/// store through a Kotlin class (added in `packaging/android`'s Gradle
/// build). It needs the JVM and the app's context to reach it, once, before
/// the first connection; without them every download fails.
///
/// A failure here is logged, not fatal: the game runs without downloads, and
/// the download screen shows each one's error.
#[cfg(target_os = "android")]
fn init_certificate_verifier(app: &bevy::android::android_activity::AndroidApp) {
    use jni::JavaVM;
    use jni::objects::JObject;

    // SAFETY: both pointers come from android-activity, valid for the
    // process's lifetime: the JavaVM it was loaded into, and a global
    // reference to the activity.
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) };
    let result = vm.attach_current_thread(|env| -> Result<(), jni::errors::Error> {
        let activity = unsafe { JObject::from_raw(env, app.activity_as_ptr().cast()) };
        rustls_platform_verifier::android::init_with_env(env, activity)
    });
    if let Err(e) = result {
        bevy::log::error!("Certificate verification is unavailable; downloads will fail: {e}");
    }
}
