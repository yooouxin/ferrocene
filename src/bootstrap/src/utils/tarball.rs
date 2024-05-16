//! Facilitates the management and generation of tarballs.
//!
//! Tarballs efficiently hold Rust compiler build artifacts and
//! capture a snapshot of each boostrap stage.
//! In uplifting, a tarball from Stage N captures essential components
//! to assemble Stage N + 1 compiler.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use crate::core::builder::Builder;
use crate::core::{build_steps::dist::distdir, builder::Kind};
use crate::utils::channel;
use crate::utils::helpers::{move_file, t};

#[derive(Copy, Clone)]
pub(crate) enum OverlayKind {
    Rust,
    Llvm,
    Cargo,
    Clippy,
    Miri,
    Rustfmt,
    RustDemangler,
    Rls,
    RustAnalyzer,
    RustcCodegenCranelift,
    LlvmBitcodeLinker,
}

impl OverlayKind {
    fn legal_and_readme(&self) -> &[&str] {
        match self {
            OverlayKind::Rust => &["COPYRIGHT", "LICENSE-APACHE", "LICENSE-MIT", "README.md"],
            OverlayKind::Llvm => {
                &["src/llvm-project/llvm/LICENSE.TXT", "src/llvm-project/llvm/README.txt"]
            }
            OverlayKind::Cargo => &[
                "src/tools/cargo/README.md",
                "src/tools/cargo/LICENSE-MIT",
                "src/tools/cargo/LICENSE-APACHE",
                "src/tools/cargo/LICENSE-THIRD-PARTY",
            ],
            OverlayKind::Clippy => &[
                "src/tools/clippy/README.md",
                "src/tools/clippy/LICENSE-APACHE",
                "src/tools/clippy/LICENSE-MIT",
            ],
            OverlayKind::Miri => &[
                "src/tools/miri/README.md",
                "src/tools/miri/LICENSE-APACHE",
                "src/tools/miri/LICENSE-MIT",
            ],
            OverlayKind::Rustfmt => &[
                "src/tools/rustfmt/README.md",
                "src/tools/rustfmt/LICENSE-APACHE",
                "src/tools/rustfmt/LICENSE-MIT",
            ],
            OverlayKind::RustDemangler => {
                &["src/tools/rust-demangler/README.md", "LICENSE-APACHE", "LICENSE-MIT"]
            }
            OverlayKind::Rls => &["src/tools/rls/README.md", "LICENSE-APACHE", "LICENSE-MIT"],
            OverlayKind::RustAnalyzer => &[
                "src/tools/rust-analyzer/README.md",
                "src/tools/rust-analyzer/LICENSE-APACHE",
                "src/tools/rust-analyzer/LICENSE-MIT",
            ],
            OverlayKind::RustcCodegenCranelift => &[
                "compiler/rustc_codegen_cranelift/Readme.md",
                "compiler/rustc_codegen_cranelift/LICENSE-APACHE",
                "compiler/rustc_codegen_cranelift/LICENSE-MIT",
            ],
            OverlayKind::LlvmBitcodeLinker => &[
                "COPYRIGHT",
                "LICENSE-APACHE",
                "LICENSE-MIT",
                "src/tools/llvm-bitcode-linker/README.md",
            ],
        }
    }

    fn version(&self, builder: &Builder<'_>) -> String {
        match self {
            OverlayKind::Rust => builder.rust_version(),
            OverlayKind::Llvm => builder.rust_version(),
            OverlayKind::RustDemangler => builder.release_num("rust-demangler"),
            OverlayKind::Cargo => {
                builder.cargo_info.version(builder, &builder.release_num("cargo"))
            }
            OverlayKind::Clippy => {
                builder.clippy_info.version(builder, &builder.release_num("clippy"))
            }
            OverlayKind::Miri => builder.miri_info.version(builder, &builder.release_num("miri")),
            OverlayKind::Rustfmt => {
                builder.rustfmt_info.version(builder, &builder.release_num("rustfmt"))
            }
            OverlayKind::Rls => builder.release(&builder.release_num("rls")),
            OverlayKind::RustAnalyzer => builder
                .rust_analyzer_info
                .version(builder, &builder.release_num("rust-analyzer/crates/rust-analyzer")),
            OverlayKind::RustcCodegenCranelift => builder.rust_version(),
            OverlayKind::LlvmBitcodeLinker => builder.rust_version(),
        }
    }
}

pub(crate) struct Tarball<'a> {
    builder: &'a Builder<'a>,

    component: String,
    target: Option<String>,
    product_name: String,
    overlay: OverlayKind,

    temp_dir: PathBuf,
    image_dir: PathBuf,
    overlay_dir: PathBuf,
    bulk_dirs: Vec<PathBuf>,

    include_target_in_component_name: bool,
    is_preview: bool,
    permit_symlinks: bool,

    proxied_binaries: Vec<String>,
    managed_prefixes: Vec<String>,
}

impl<'a> Tarball<'a> {
    pub(crate) fn new(builder: &'a Builder<'a>, component: &str, target: &str) -> Self {
        Self::new_inner(builder, component, Some(target.into()))
    }

    pub(crate) fn new_targetless(builder: &'a Builder<'a>, component: &str) -> Self {
        Self::new_inner(builder, component, None)
    }

    fn new_inner(builder: &'a Builder<'a>, component: &str, target: Option<String>) -> Self {
        let mut temp_dir = builder.out.join("tmp").join("tarball").join(component);
        if let Some(target) = &target {
            temp_dir = temp_dir.join(target);
        }
        let _ = std::fs::remove_dir_all(&temp_dir);

        let image_dir = temp_dir.join("image");
        let overlay_dir = temp_dir.join("overlay");

        Self {
            builder,

            component: component.into(),
            target,
            product_name: "Rust".into(),
            overlay: OverlayKind::Rust,

            temp_dir,
            image_dir,
            overlay_dir,
            bulk_dirs: Vec::new(),

            include_target_in_component_name: false,
            is_preview: false,
            permit_symlinks: false,

            proxied_binaries: Vec::new(),
            managed_prefixes: Vec::new(),
        }
    }

    /// Register a binary that should have a proxy created by criticalup.
    pub(crate) fn ferrocene_proxied_binary(&mut self, path: &str) {
        self.proxied_binaries.push(path.into());
    }

    /// Register a path prefix that criticalup should prevent non-Ferrocene files from existing
    /// into it.
    pub(crate) fn ferrocene_managed_prefix(&mut self, path: &str) {
        self.managed_prefixes.push(path.into());
    }

    pub(crate) fn set_overlay(&mut self, overlay: OverlayKind) {
        self.overlay = overlay;
    }

    pub(crate) fn set_product_name(&mut self, name: &str) {
        self.product_name = name.into();
    }

    pub(crate) fn include_target_in_component_name(&mut self, include: bool) {
        self.include_target_in_component_name = include;
    }

    pub(crate) fn is_preview(&mut self, is: bool) {
        self.is_preview = is;
    }

    pub(crate) fn permit_symlinks(&mut self, flag: bool) {
        self.permit_symlinks = flag;
    }

    pub(crate) fn image_dir(&self) -> &Path {
        t!(std::fs::create_dir_all(&self.image_dir));
        &self.image_dir
    }

    pub(crate) fn add_file(&self, src: impl AsRef<Path>, destdir: impl AsRef<Path>, perms: u32) {
        // create_dir_all fails to create `foo/bar/.`, so when the destination is "." this simply
        // uses the base directory as the destination directory.
        let destdir = if destdir.as_ref() == Path::new(".") {
            self.image_dir.clone()
        } else {
            self.image_dir.join(destdir.as_ref())
        };

        t!(std::fs::create_dir_all(&destdir));
        self.builder.install(src.as_ref(), &destdir, perms);
    }

    pub(crate) fn add_renamed_file(
        &self,
        src: impl AsRef<Path>,
        destdir: impl AsRef<Path>,
        new_name: &str,
    ) {
        let destdir = self.image_dir.join(destdir.as_ref());
        t!(std::fs::create_dir_all(&destdir));
        self.builder.copy_link(src.as_ref(), &destdir.join(new_name));
    }

    pub(crate) fn add_legal_and_readme_to(&self, destdir: impl AsRef<Path>) {
        for file in self.overlay.legal_and_readme() {
            self.add_file(self.builder.src.join(file), destdir.as_ref(), 0o644);
        }
    }

    pub(crate) fn add_dir(&self, src: impl AsRef<Path>, dest: impl AsRef<Path>) {
        let dest = self.image_dir.join(dest.as_ref());

        t!(std::fs::create_dir_all(&dest));
        self.builder.cp_link_r(src.as_ref(), &dest);
    }

    pub(crate) fn add_bulk_dir(&mut self, src: impl AsRef<Path>, dest: impl AsRef<Path>) {
        self.bulk_dirs.push(dest.as_ref().to_path_buf());
        self.add_dir(src, dest);
    }

    pub(crate) fn generate(self) -> GeneratedTarball {
        let mut component_name = self.component.clone();
        if self.is_preview {
            component_name.push_str("-preview");
        }
        if self.include_target_in_component_name {
            component_name.push('-');
            component_name.push_str(
                self.target
                    .as_ref()
                    .expect("include_target_in_component_name used in a targetless tarball"),
            );
        }

        self.run(|this, cmd| {
            cmd.arg("generate")
                .arg("--image-dir")
                .arg(&this.image_dir)
                .arg(format!("--component-name={}", &component_name));

            if let Some((dir, dirs)) = this.bulk_dirs.split_first() {
                let mut arg = dir.as_os_str().to_os_string();
                for dir in dirs {
                    arg.push(",");
                    arg.push(dir);
                }
                cmd.arg("--bulk-dirs").arg(&arg);
            }

            this.non_bare_args(cmd);
        })
    }

    pub(crate) fn combine(self, tarballs: &[GeneratedTarball]) -> GeneratedTarball {
        let mut input_tarballs = tarballs[0].path.as_os_str().to_os_string();
        for tarball in &tarballs[1..] {
            input_tarballs.push(",");
            input_tarballs.push(&tarball.path);
        }

        self.run(|this, cmd| {
            cmd.arg("combine").arg("--input-tarballs").arg(input_tarballs);
            this.non_bare_args(cmd);
        })
    }

    pub(crate) fn bare(self) -> GeneratedTarball {
        // Bare tarballs should have the top level directory match the package
        // name, not "image". We rename the image directory just before passing
        // into rust-installer.
        let dest = self.temp_dir.join(self.package_name());
        t!(move_file(&self.image_dir, &dest));

        self.run(|this, cmd| {
            let distdir = distdir(this.builder);
            t!(std::fs::create_dir_all(&distdir));
            cmd.arg("tarball")
                .arg("--input")
                .arg(&dest)
                .arg("--output")
                .arg(distdir.join(this.package_name()));
        })
    }

    /// Ferrocene uses a custom package naming scheme, with the component name, the optional target
    /// name, and the version. The version is:
    ///
    /// * For stable releases, the version number.
    /// * For any other build where we have git information, the short commit hash.
    /// * Otherwise, the string "custom".
    ///
    /// See <https://designs.ferrocene.dev/tarball-names.html>
    fn package_name(&self) -> String {
        let mut name = self.component.clone();
        if let Some(target) = &self.target {
            name.push('-');
            name.push_str(&target);
        }
        name.push('-');
        if self.builder.config.ferrocene_raw_channel == "stable" {
            name.push_str(
                self.builder.read(&self.builder.src.join("ferrocene").join("version")).trim(),
            );
        } else if let Some(sha) = self.builder.rust_info().sha_short() {
            name.push_str(sha);
        } else {
            name.push_str("custom");
        }
        name
    }

    fn non_bare_args(&self, cmd: &mut Command) {
        cmd.arg("--rel-manifest-dir=rustlib")
            .arg("--legacy-manifest-dirs=rustlib,cargo")
            .arg(format!("--product-name={}", self.product_name))
            .arg(format!("--success-message={} installed.", self.component))
            .arg(format!("--package-name={}", self.package_name()))
            .arg("--non-installed-overlay")
            .arg(&self.overlay_dir)
            .arg("--output-dir")
            .arg(distdir(self.builder));

        cmd.arg("--ferrocene-component").arg(match &self.target {
            Some(target) => format!("{}-{target}", self.component),
            None => self.component.clone(),
        });
        if let Some(sha) = self.builder.rust_sha() {
            cmd.arg("--ferrocene-commit-sha").arg(sha);
        }
        if let Some(kms_key_arn) = &self.builder.config.ferrocene_tarball_signing_kms_key_arn {
            cmd.arg("--ferrocene-signing-kms-key-arn").arg(kms_key_arn);
        }
        for proxied_binary in &self.proxied_binaries {
            cmd.arg("--ferrocene-proxied-binary").arg(proxied_binary);
        }
        for managed_prefix in &self.managed_prefixes {
            cmd.arg("--ferrocene-managed-prefix").arg(managed_prefix);
        }
    }

    fn run(self, build_cli: impl FnOnce(&Tarball<'a>, &mut Command)) -> GeneratedTarball {
        t!(std::fs::create_dir_all(&self.overlay_dir));
        self.builder.create(&self.overlay_dir.join("version"), &self.overlay.version(self.builder));
        if let Some(info) = self.builder.rust_info().info() {
            channel::write_commit_hash_file(&self.overlay_dir, &info.sha);
            channel::write_commit_info_file(&self.overlay_dir, info);
        }
        for file in self.overlay.legal_and_readme() {
            self.builder.install(&self.builder.src.join(file), &self.overlay_dir, 0o644);
        }

        let mut cmd =
            self.builder.tool_cmd(crate::core::build_steps::tool::Tool::FerroceneGenerateTarball);

        let package_name = self.package_name();
        self.builder.info(&format!("Dist {package_name}"));
        let _time = crate::utils::helpers::timeit(self.builder);

        build_cli(&self, &mut cmd);
        cmd.arg("--work-dir").arg(&self.temp_dir);
        if let Some(formats) = &self.builder.config.dist_compression_formats {
            assert!(!formats.is_empty(), "dist.compression-formats can't be empty");
            cmd.arg("--compression-formats").arg(formats.join(","));
        }

        // For `x install` tarball files aren't needed, so we can speed up the process by not producing them.
        let compression_profile = if self.builder.kind == Kind::Install {
            self.builder.verbose(|| {
                println!("Forcing dist.compression-profile = 'no-op' for `x install`.")
            });
            // "no-op" indicates that the rust-installer won't produce compressed tarball sources.
            "no-op"
        } else {
            assert!(
                self.builder.config.dist_compression_profile != "no-op",
                "dist.compression-profile = 'no-op' can only be used for `x install`"
            );

            &self.builder.config.dist_compression_profile
        };

        cmd.args(["--compression-profile", compression_profile]);
        self.builder.run(&mut cmd);

        // Ensure there are no symbolic links in the tarball. In particular,
        // rustup-toolchain-install-master and most versions of Windows can't handle symbolic links.
        let decompressed_output = self.temp_dir.join(&package_name);
        if !self.builder.config.dry_run() && !self.permit_symlinks {
            for entry in walkdir::WalkDir::new(&decompressed_output) {
                let entry = t!(entry);
                if entry.path_is_symlink() {
                    panic!("generated a symlink in a tarball: {}", entry.path().display());
                }
            }
        }

        // Use either the first compression format defined, or "gz" as the default.
        let ext = self
            .builder
            .config
            .dist_compression_formats
            .as_ref()
            .and_then(|formats| formats.first())
            .map(|s| s.as_str())
            .unwrap_or("gz");

        GeneratedTarball {
            path: distdir(self.builder).join(format!("{package_name}.tar.{ext}")),
            decompressed_output,
            work: self.temp_dir,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GeneratedTarball {
    path: PathBuf,
    decompressed_output: PathBuf,
    work: PathBuf,
}

impl GeneratedTarball {
    pub(crate) fn tarball(&self) -> &Path {
        &self.path
    }

    pub(crate) fn decompressed_output(&self) -> &Path {
        &self.decompressed_output
    }

    pub(crate) fn work_dir(&self) -> &Path {
        &self.work
    }
}
