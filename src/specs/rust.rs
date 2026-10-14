use super::gens;
use crate::spec::{Opt, Subcommand, arg, cmd, opt};

fn common() -> Vec<Opt> {
    vec![
        opt("-p, --package", "Package to operate on").repeatable().arg(arg("spec").generator(gens::cargo_packages())),
        opt("--workspace, --all", "All packages in the workspace"),
        opt("--exclude", "Exclude packages").arg(arg("spec").generator(gens::cargo_packages())),
        opt("-F, --features", "Space or comma separated list of features").arg(arg("features").generator(gens::cargo_features())),
        opt("--all-features", "Activate all available features"),
        opt("--no-default-features", "Do not activate the default feature"),
        opt("-r, --release", "Build in release mode"),
        opt("--profile", "Build with the given profile").arg(arg("profile").suggest(&["dev", "release", "test", "bench"])),
        opt("--target", "Build for the target triple").arg(arg("triple").generator(gens::rustup_list("rustup:targets-installed", "target", true))),
        opt("-j, --jobs", "Number of parallel jobs").arg(arg("n")),
        opt("--manifest-path", "Path to Cargo.toml").arg(arg("path").files()),
        opt("--target-dir", "Directory for all generated artifacts").arg(arg("dir").folders()),
        opt("--locked", "Assert that Cargo.lock will remain unchanged"),
        opt("--offline", "Run without accessing the network"),
        opt("--frozen", "Equivalent to --locked and --offline"),
        opt("-q, --quiet", "Do not print cargo log messages"),
        opt("-v, --verbose", "Use verbose output").repeatable(),
        opt("--message-format", "Error format").arg(arg("fmt").suggest(&["human", "short", "json"])),
    ]
}

fn targets() -> Vec<Opt> {
    vec![
        opt("--lib", "Only this package's library"),
        opt("--bin", "Only the specified binary").arg(arg("name").optional().generator(gens::cargo_targets("bin"))),
        opt("--bins", "All binaries"),
        opt("--example", "Only the specified example").arg(arg("name").optional().generator(gens::cargo_targets("example"))),
        opt("--examples", "All examples"),
        opt("--test", "Only the specified test target").arg(arg("name").optional().generator(gens::cargo_targets("test"))),
        opt("--tests", "All tests"),
        opt("--bench", "Only the specified bench target").arg(arg("name").optional().generator(gens::cargo_targets("bench"))),
        opt("--benches", "All benches"),
        opt("--all-targets", "All targets"),
    ]
}

pub fn cargo() -> Subcommand {
    cmd("cargo", "Rust's package manager")
        .opts([
            opt("-V, --version", "Print version info"),
            opt("--list", "List installed commands"),
            opt("-C", "Change to DIRECTORY before doing anything").arg(arg("dir").folders()),
            opt("-Z", "Unstable (nightly-only) flags").arg(arg("flag")),
        ])
        .subs([
            cmd("build, b", "Compile the current package").opts(common()).opts(targets()),
            cmd("check, c", "Analyze the current package and report errors").opts(common()).opts(targets()),
            cmd("run, r", "Run a binary or example of the local package")
                .opts(common())
                .opts([
                    opt("--bin", "Name of the bin target to run").arg(arg("name").generator(gens::cargo_targets("bin"))),
                    opt("--example", "Name of the example target to run").arg(arg("name").generator(gens::cargo_targets("example"))),
                ])
                .arg(arg("args").optional().variadic().files()),
            cmd("test, t", "Run the tests")
                .opts(common())
                .opts(targets())
                .opts([opt("--doc", "Test only this library's documentation"), opt("--no-run", "Compile, but don't run tests"), opt("--no-fail-fast", "Run all tests regardless of failure")])
                .arg(arg("testname").optional())
                .arg(arg("args").optional().variadic().suggest_desc(&[
                    ("--nocapture", "Don't capture stdout/stderr"),
                    ("--ignored", "Run only ignored tests"),
                    ("--include-ignored", "Run ignored and not ignored tests"),
                    ("--exact", "Exactly match filters"),
                    ("--test-threads", "Number of threads"),
                ])),
            cmd("bench", "Run the benchmarks").opts(common()).opts(targets()).arg(arg("benchname").optional()),
            cmd("clippy", "Check a package to catch common mistakes")
                .opts(common())
                .opts(targets())
                .opts([opt("--fix", "Automatically apply lint suggestions"), opt("--allow-dirty", "Fix code even if the working directory is dirty"), opt("--no-deps", "Run only on the given crate")])
                .arg(arg("args").optional().variadic().suggest_desc(&[("--", "Pass options to clippy"), ("-D", "Deny a lint"), ("-W", "Warn about a lint")])),
            cmd("fmt", "Format all Rust files of the current crate")
                .opts([opt("--check", "Run in check mode"), opt("--all", "Format all packages"), opt("-p, --package", "Package").arg(arg("spec").generator(gens::cargo_packages()))]),
            cmd("doc, d", "Build this package's documentation").opts(common()).opts([opt("--open", "Open the docs in a browser"), opt("--no-deps", "Don't build docs for dependencies"), opt("--document-private-items", "Document private items")]),
            cmd("new", "Create a new cargo package")
                .opts([opt("--bin", "Use a binary template (default)"), opt("--lib", "Use a library template"), opt("--name", "Package name").arg(arg("name")), opt("--vcs", "VCS").arg(arg("vcs").suggest(&["git", "none"])), opt("--edition", "Edition").arg(arg("year").suggest(&["2024", "2021", "2018"]))])
                .arg(arg("path").folders()),
            cmd("init", "Create a new cargo package in an existing directory")
                .opts([opt("--bin", "Use a binary template"), opt("--lib", "Use a library template"), opt("--name", "Package name").arg(arg("name"))])
                .arg(arg("path").optional().folders()),
            cmd("add", "Add dependencies to a Cargo.toml manifest file")
                .opts([
                    opt("--dev", "Add as a development dependency"),
                    opt("--build", "Add as a build dependency"),
                    opt("-F, --features", "Features to activate").arg(arg("features")),
                    opt("--no-default-features", "Disable the default features"),
                    opt("--optional", "Mark the dependency as optional"),
                    opt("--path", "Local crate path").arg(arg("path").folders()),
                    opt("--git", "Git repository location").arg(arg("uri")),
                    opt("--branch", "Git branch").arg(arg("branch")),
                    opt("--rename", "Rename the dependency").arg(arg("name")),
                    opt("-p, --package", "Package to modify").arg(arg("spec").generator(gens::cargo_packages())),
                    opt("--dry-run", "Don't actually write the manifest"),
                ])
                .arg(arg("crate").variadic().generator(gens::crates_io())),
            cmd("remove, rm", "Remove dependencies from a Cargo.toml manifest file")
                .opts([opt("--dev", "Remove from dev-dependencies"), opt("--build", "Remove from build-dependencies"), opt("-p, --package", "Package to modify").arg(arg("spec").generator(gens::cargo_packages()))])
                .arg(arg("dependency").variadic().generator(crate::spec::gen_fn("cargo:deps", |cx| {
                    let Some(m) = gens::find_up(cx.cwd, "Cargo.toml") else { return vec![] };
                    let v = std::fs::read_to_string(m).ok().and_then(|t| t.parse::<toml::Table>().ok()).unwrap_or_default();
                    let mut out = vec![];
                    for (sec, d) in [("dependencies", "dependency"), ("dev-dependencies", "dev dependency"), ("build-dependencies", "build dependency")] {
                        if let Some(t) = v.get(sec).and_then(|x| x.as_table()) {
                            out.extend(t.keys().map(|k| crate::spec::sug(k.clone()).desc(d)));
                        }
                    }
                    out
                }))),
            cmd("install", "Install a Rust binary")
                .opts([
                    opt("--path", "Filesystem path to local crate").arg(arg("path").folders()),
                    opt("--git", "Git URL to install from").arg(arg("url")),
                    opt("--version", "Version to install").arg(arg("version")),
                    opt("-f, --force", "Force overwriting existing crates"),
                    opt("--locked", "Use Cargo.lock"),
                    opt("--root", "Directory to install packages into").arg(arg("dir").folders()),
                    opt("--list", "List all installed packages"),
                    opt("-F, --features", "Features").arg(arg("features")),
                ])
                .arg(arg("crate").optional().variadic().generator(gens::crates_io())),
            cmd("uninstall", "Remove a Rust binary").arg(arg("spec").variadic().generator(gens::cargo_installed())),
            cmd("update", "Update dependencies listed in Cargo.lock")
                .opts([opt("-p, --package", "Package to update").arg(arg("spec")), opt("--dry-run", "Don't write the lockfile"), opt("--precise", "Update to exactly this version").arg(arg("version"))])
                .arg(arg("spec").optional().variadic()),
            cmd("publish", "Upload a package to the registry").opts([opt("--dry-run", "Perform all checks without uploading"), opt("--allow-dirty", "Allow dirty working directories"), opt("--no-verify", "Don't verify the contents by building them")]),
            cmd("clean", "Remove the target directory").opts([opt("-r, --release", "Only the release directory"), opt("--doc", "Only the doc directory"), opt("-p, --package", "Package to clean").arg(arg("spec").generator(gens::cargo_packages()))]),
            cmd("tree", "Display a tree visualization of a dependency graph")
                .opts([opt("-i, --invert", "Invert the tree for a package").arg(arg("spec")), opt("-d, --duplicates", "Show only duplicated dependencies"), opt("--depth", "Maximum display depth").arg(arg("n")), opt("-e, --edges", "Kinds of dependencies").arg(arg("kinds").suggest(&["normal", "build", "dev", "all", "features"]))]),
            cmd("search", "Search packages in crates.io").opt(opt("--limit", "Limit results").arg(arg("n"))).arg(arg("query")),
            cmd("fix", "Automatically fix lint warnings reported by rustc").opts(common()).opts([opt("--edition", "Fix in preparation for the next edition"), opt("--allow-dirty", "Fix code even if the working directory is dirty")]),
            cmd("metadata", "Output the resolved dependencies of a package").opts([opt("--no-deps", "Only workspace members"), opt("--format-version", "Format version").arg(arg("v").suggest(&["1"]))]),
            cmd("vendor", "Vendor all dependencies locally").arg(arg("path").optional().folders()),
            cmd("login", "Log in to a registry").arg(arg("token").optional()),
            cmd("package", "Assemble the local package into a distributable tarball").opts([opt("--list", "Print files included"), opt("--allow-dirty", "Allow dirty working directories")]),
            cmd("generate-lockfile", "Generate the lockfile for a package"),
            cmd("help", "Displays help for a cargo subcommand").arg(arg("command").optional()),
        ])
}

pub fn rustup() -> Subcommand {
    let toolchain = || arg("toolchain").generator(gens::rustup_toolchains()).suggest(&["stable", "beta", "nightly"]);
    cmd("rustup", "The Rust toolchain installer").subs([
        cmd("update", "Update Rust toolchains and rustup").arg(toolchain().optional().variadic()),
        cmd("default", "Set the default toolchain").arg(toolchain().optional()),
        cmd("show", "Show the active and installed toolchains").subs([cmd("active-toolchain", "Show the active toolchain"), cmd("home", "Display the rustup home")]),
        cmd("toolchain", "Modify or query the installed toolchains").subs([
            cmd("list", "List installed toolchains"),
            cmd("install", "Install a toolchain").opt(opt("--profile", "Profile").arg(arg("profile").suggest(&["minimal", "default", "complete"]))).arg(toolchain().variadic()),
            cmd("uninstall", "Uninstall a toolchain").arg(arg("toolchain").variadic().generator(gens::rustup_toolchains())),
            cmd("link", "Create a custom toolchain from a directory").arg(arg("name")).arg(arg("path").folders()),
        ]),
        cmd("target", "Modify a toolchain's supported targets").subs([
            cmd("list", "List installed and available targets").opt(opt("--installed", "List only installed targets")),
            cmd("add", "Add a target").opt(opt("--toolchain", "Toolchain").arg(toolchain())).arg(arg("target").variadic().generator(gens::rustup_list("rustup:targets", "target", false))),
            cmd("remove", "Remove a target").arg(arg("target").variadic().generator(gens::rustup_list("rustup:targets-installed", "target", true))),
        ]),
        cmd("component", "Modify a toolchain's installed components").subs([
            cmd("list", "List installed and available components").opt(opt("--installed", "List only installed components")),
            cmd("add", "Add a component").opt(opt("--toolchain", "Toolchain").arg(toolchain())).arg(arg("component").variadic().generator(gens::rustup_list("rustup:components", "component", false))),
            cmd("remove", "Remove a component").arg(arg("component").variadic().generator(gens::rustup_list("rustup:components-installed", "component", true))),
        ]),
        cmd("override", "Modify toolchain overrides for directories").subs([
            cmd("set", "Set the override toolchain for a directory").arg(toolchain()),
            cmd("unset", "Remove the override toolchain for a directory"),
            cmd("list", "List directory toolchain overrides"),
        ]),
        cmd("run", "Run a command with an environment configured for a toolchain").arg(toolchain()).arg(arg("command").command()),
        cmd("which", "Display which binary will be run for a command").arg(arg("command")),
        cmd("doc", "Open the documentation for the current toolchain").opts([opt("--std", "Standard library API"), opt("--book", "The Rust Programming Language book")]),
        cmd("self", "Modify the rustup installation").subs([cmd("update", "Download and install updates to rustup"), cmd("uninstall", "Uninstall rustup")]),
        cmd("completions", "Generate tab-completion scripts").arg(arg("shell").suggest(&["bash", "zsh", "fish"])),
    ])
}
