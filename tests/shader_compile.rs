// SPDX-License-Identifier: MIT

//! Compiles every `assets/shaders/*.wesl` the way Bevy does, so a broken
//! shader fails `cargo test` instead of the first frame that needs it.
//!
//! A shader is an *asset*: nothing about it is checked by `cargo build`.
//! `ShaderRef` is a plain string, the source is parsed at runtime, and the
//! failure arrives as a wgpu validation error the moment some material's
//! pipeline is first specialized — which for the 3D note tail means
//! entering Play 3D, and for the tie material means playing a song with a
//! note crossing a bar line. The Bevy 0.20 port hit exactly this twice.
//!
//! This runs the same compiler Bevy runs — `wesl`, already in the lockfile
//! via `bevy_shader`, so it costs the build nothing (same reasoning as
//! `skrifa` in `tests/glyph_coverage.rs`) — with `validate: true`, so it
//! catches syntax errors, unknown identifiers, bad types and wrong arity,
//! not merely a parse failure.
//!
//! **What it cannot catch**, and why the sibling checks in
//! `tests/asset_layout.rs` still matter: the Bevy modules our shaders
//! import live inside the Bevy crates, at a path that moves with every
//! checkout, so they are stubbed below. A stub that drifts from the real
//! struct — Bevy renaming a field we don't use, say — goes unnoticed here.
//! Using a field the stub *lacks* does fail, which is what keeps the stubs
//! honest in the direction that matters.

use std::borrow::Cow;
use std::path::Path;

use wesl::syntax::{ModulePath, PathOrigin};
use wesl::{CompileOptions, ResolveError, Resolver, Wesl};

/// Stand-in for `bevy_ui_render::ui_vertex_output`, copied from
/// `crates/bevy_ui_render/src/ui_vertex_output.wesl`. Six of our seven
/// shaders are `UiMaterial`s and take this as their fragment input.
const UI_VERTEX_OUTPUT: &str = "\
struct UiVertexOutput {
    @location(0) uv: vec2<f32>,
    @location(1) border_widths: vec4<f32>,
    @location(2) border_radius_x: vec4<f32>,
    @location(3) border_radius_y: vec4<f32>,
    @location(4) @interpolate(flat) size: vec2<f32>,
    @builtin(position) position: vec4<f32>,
};
";

/// Stand-in for `bevy_pbr::render::forward_io`, used by the one mesh
/// `Material` we have (`note_ribbon_3d`). The real struct gates several
/// fields behind `@if(...)` shader defs; only the ones a mesh material
/// can always rely on are declared here.
const FORWARD_IO: &str = "\
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};
";

/// Stand-in for the virtual `constants` package Bevy's shader cache
/// synthesises from a pipeline's `ShaderDefVal`s (see `bevy_shader`'s
/// `ShaderResolver`). `MATERIAL_BIND_GROUP` is pushed by
/// `bevy_pbr::material`; the value matters only in that it type-checks.
const CONSTANTS: &str = "const MATERIAL_BIND_GROUP: u32 = 3u;\n";

fn shader_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/shaders")
}

/// Serves our own shaders from `assets/shaders/`, and the Bevy modules
/// they import from the stubs above.
///
/// `inline` overrides the on-disk lookup for one module name, which is how
/// [`the_compiler_rejects_broken_shaders`] feeds it a deliberately broken
/// source without writing a bad file into `assets/`.
struct StubResolver {
    inline: Option<(String, String)>,
}

impl StubResolver {
    fn from_disk() -> Self {
        Self { inline: None }
    }

    fn with_inline(name: &str, source: &str) -> Self {
        Self { inline: Some((name.to_string(), source.to_string())) }
    }
}

impl Resolver for StubResolver {
    fn resolve_source<'a>(&'a self, path: &ModulePath) -> Result<Cow<'a, str>, ResolveError> {
        let parts: Vec<&str> = path.components.iter().map(String::as_str).collect();
        if let (Some((name, source)), [_, requested]) = (&self.inline, parts.as_slice())
            && name == requested
        {
            return Ok(Cow::Borrowed(source));
        }
        match (&path.origin, parts.as_slice()) {
            (PathOrigin::Package(p), ["ui_vertex_output"]) if p == "bevy_ui_render" => {
                Ok(Cow::Borrowed(UI_VERTEX_OUTPUT))
            }
            (PathOrigin::Package(p), ["render", "forward_io"]) if p == "bevy_pbr" => {
                Ok(Cow::Borrowed(FORWARD_IO))
            }
            (PathOrigin::Package(p), []) if p == "constants" => Ok(Cow::Borrowed(CONSTANTS)),
            // Our own shaders, addressed as `package::shaders::<name>`.
            (PathOrigin::Absolute, ["shaders", name]) => {
                let file = shader_dir().join(format!("{name}.wesl"));
                std::fs::read_to_string(&file)
                    .map(Cow::Owned)
                    .map_err(|e| ResolveError::FileNotFound(file, e.to_string()))
            }
            _ => Err(ResolveError::ModuleNotFound(
                path.clone(),
                "not one of this repo's shaders, and not a stubbed Bevy module — \
                 add a stub in tests/shader_compile.rs if the import is legitimate"
                    .to_string(),
            )),
        }
    }
}

fn compiler(resolver: StubResolver) -> Wesl<StubResolver> {
    let mut wesl = Wesl::new_barebones().set_custom_resolver(resolver);
    // `validate` is the point of this test: without it a shader only has to
    // *parse*, and a typo'd identifier or a wrong type sails through.
    wesl.set_options(CompileOptions {
        imports: true,
        condcomp: true,
        validate: true,
        ..Default::default()
    });
    wesl
}

fn module(name: &str) -> ModulePath {
    ModulePath::new(PathOrigin::Absolute, vec!["shaders".to_string(), name.to_string()])
}

fn shader_names() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(shader_dir())
        .expect("assets/shaders must exist")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("wesl"))
        .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .collect();
    names.sort();
    names
}

#[test]
fn every_shader_compiles() {
    let names = shader_names();
    assert!(!names.is_empty(), "found no .wesl shaders to compile");

    let wesl = compiler(StubResolver::from_disk());
    let mut report = String::new();
    for name in &names {
        if let Err(err) = wesl.compile(&module(name)) {
            report.push_str(&format!("\n── {name}.wesl ──\n{err}\n"));
        }
    }
    assert!(
        report.is_empty(),
        "{} of {} shaders failed to compile:\n{report}",
        report.matches("──").count() / 2,
        names.len()
    );
}

/// The compiler above must actually reject a bad shader.
///
/// Both faults this test exists to catch were, at one point, "caught" by a
/// check that could not fail — so prove the pipeline rejects each of the
/// two shapes that really broke: a leftover naga_oil directive, and an
/// identifier that doesn't exist.
#[test]
fn the_compiler_rejects_broken_shaders() {
    for (label, source) in [
        // naga_oil's substitution syntax, which is what survived the first
        // sweep of the port and failed at runtime in Play 3D.
        (
            "naga_oil substitution",
            "@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> c: vec4<f32>;",
        ),
        // naga_oil's import syntax.
        ("naga_oil import", "#import bevy_ui_render::ui_vertex_output::UiVertexOutput\n"),
        // A plain typo: no such function.
        (
            "unknown identifier",
            "@fragment fn fragment() -> @location(0) vec4<f32> { return no_such_fn(1.0); }",
        ),
    ] {
        let wesl = compiler(StubResolver::with_inline("broken", source));
        assert!(
            wesl.compile(&module("broken")).is_err(),
            "the shader compiler accepted a shader it should have rejected \
             ({label}): this test's own guarantee is void"
        );
    }
}
