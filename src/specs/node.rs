use super::gens;
use crate::spec::{Arg, Subcommand, arg, cmd, opt};

fn script() -> Arg {
    arg("script").generator(gens::npm_scripts())
}

fn dep() -> Arg {
    arg("package").variadic().generator(gens::npm_deps())
}

pub fn npm() -> Subcommand {
    let ws = || opt("-w, --workspace", "Run in a workspace").repeatable().arg(arg("name").folders());
    cmd("npm", "Node package manager").opts([opt("-v, --version", "Print version"), opt("-g, --global", "Operate globally").persistent()]).subs([
        cmd("install, i, add", "Install a package")
            .opts([
                opt("-D, --save-dev", "Save to devDependencies"),
                opt("-P, --save-prod", "Save to dependencies"),
                opt("-O, --save-optional", "Save to optionalDependencies"),
                opt("-E, --save-exact", "Save an exact version"),
                opt("--no-save", "Don't save to package.json"),
                opt("--legacy-peer-deps", "Ignore peer dependency conflicts"),
                opt("--force", "Force fetching remote resources"),
                ws(),
            ])
            .arg(arg("package").optional().variadic().folders()),
        cmd("ci, clean-install", "Clean install from the lockfile").opt(opt("--omit", "Omit dependency types").arg(arg("type").suggest(&["dev", "optional", "peer"]))),
        cmd("uninstall, remove, rm, un", "Remove a package").opts([opt("-D, --save-dev", "From devDependencies"), ws()]).arg(dep()),
        cmd("update, up, upgrade", "Update packages").arg(dep().optional()),
        cmd("run, run-script", "Run a package.json script").opts([ws(), opt("--if-present", "Don't fail if the script is missing"), opt("--workspaces", "Run in all workspaces")]).arg(script()).arg(arg("args").optional().variadic()),
        cmd("test, t", "Run the test script"),
        cmd("start", "Run the start script"),
        cmd("stop", "Run the stop script"),
        cmd("restart", "Run the restart script"),
        cmd("init, create", "Create a package.json file").opts([opt("-y, --yes", "Use defaults"), opt("--scope", "Scope").arg(arg("scope"))]).arg(arg("initializer").optional()),
        cmd("exec, x", "Run a command from a local or remote package").opts([opt("-p, --package", "Package to install").arg(arg("package")), opt("-c, --call", "Script string").arg(arg("script"))]).arg(arg("command").command()),
        cmd("outdated", "Check for outdated packages").arg(dep().optional()),
        cmd("ls, list", "List installed packages").opts([opt("--depth", "Depth").arg(arg("n")), opt("--all", "Show all dependencies")]).arg(dep().optional()),
        cmd("audit", "Run a security audit").subs([cmd("fix", "Fix vulnerabilities").opt(opt("--force", "Allow breaking changes")), cmd("signatures", "Verify signatures")]),
        cmd("publish", "Publish a package").opts([opt("--access", "Access level").arg(arg("level").suggest(&["public", "restricted"])), opt("--tag", "Dist tag").arg(arg("tag")), opt("--dry-run", "Dry run")]).arg(arg("folder").optional().folders()),
        cmd("version", "Bump a package version").arg(arg("type").suggest(&["patch", "minor", "major", "prepatch", "preminor", "premajor", "prerelease"])),
        cmd("link, ln", "Symlink a package folder").arg(arg("package").optional().folders()),
        cmd("pack", "Create a tarball from a package").opt(opt("--dry-run", "Dry run")),
        cmd("view, info, show", "View registry info").arg(arg("package")).arg(arg("field").optional()),
        cmd("search, s, find", "Search for packages").arg(arg("terms").variadic()),
        cmd("cache", "Manipulate the packages cache").subs([cmd("clean", "Delete all data").opt(opt("--force", "Force")), cmd("verify", "Verify the cache"), cmd("ls", "List cached packages")]),
        cmd("config, c", "Manage the npm configuration").subs([
            cmd("set", "Set a value").arg(arg("key=value")),
            cmd("get", "Get a value").arg(arg("key")),
            cmd("delete", "Delete a key").arg(arg("key")),
            cmd("list, ls", "List all config").opt(opt("-l, --long", "Show defaults")),
            cmd("edit", "Open the config file"),
        ]),
        cmd("login, adduser", "Log in to a registry"),
        cmd("logout", "Log out of the registry"),
        cmd("whoami", "Display the npm username"),
        cmd("prune", "Remove extraneous packages"),
        cmd("dedupe, ddp", "Reduce duplication in the package tree"),
        cmd("doctor", "Check the npm environment"),
        cmd("fund", "Retrieve funding information"),
        cmd("help", "Get help on npm").arg(arg("term").optional()),
    ])
}

/// pnpm / yarn / bun: subcommands plus `<tool> <script>` shorthand.
pub fn pm(name: &str) -> Subcommand {
    let dev = opt("-D, --dev", "Add as a dev dependency");
    cmd(name, "Node package manager")
        .opts([opt("-v, --version", "Print version"), opt("-C, --dir, --cwd", "Run in this directory").persistent().arg(arg("dir").folders())])
        .subs([
            cmd("add", "Add packages").opts([dev, opt("-E, --exact", "Install exact versions"), opt("-g, --global", "Install globally"), opt("-O, --optional", "Add as optional dependency")]).arg(arg("package").variadic()),
            cmd("install, i", "Install all dependencies").opts([opt("--frozen-lockfile", "Don't update the lockfile"), opt("--prod, --production", "Only production dependencies")]),
            cmd("remove, rm, uninstall", "Remove packages").opt(opt("-g, --global", "Remove globally")).arg(dep()),
            cmd("update, up, upgrade", "Update packages").opts([opt("-L, --latest", "Ignore version ranges"), opt("-i, --interactive", "Interactive")]).arg(dep().optional()),
            cmd("run", "Run a package.json script").arg(script()).arg(arg("args").optional().variadic()),
            cmd("exec", "Run a command in the project context").arg(arg("command").command()),
            cmd("dlx, x", "Run a package without installing it").arg(arg("package")),
            cmd("create", "Create a project from a starter kit").arg(arg("template")),
            cmd("init", "Create a package.json file"),
            cmd("test, t", "Run the test script"),
            cmd("start", "Run the start script"),
            cmd("build", "Run the build script"),
            cmd("dev", "Run the dev script"),
            cmd("outdated", "Check for outdated packages").arg(dep().optional()),
            cmd("why", "Show why a package is installed").arg(dep()),
            cmd("list, ls", "List installed packages").opt(opt("--depth", "Depth").arg(arg("n"))),
            cmd("link", "Link a local package").arg(arg("dir").optional().folders()),
            cmd("publish", "Publish a package").arg(arg("dir").optional().folders()),
            cmd("audit", "Check for known vulnerabilities"),
            cmd("store", "Manage the package store").subs([cmd("prune", "Remove unreferenced packages"), cmd("path", "Print the store path"), cmd("status", "Check for modified packages")]),
        ])
        .arg(arg("script").optional().generator(gens::npm_scripts()))
}

pub fn npx() -> Subcommand {
    cmd("npx", "Run a command from a local or remote npm package")
        .opts([opt("-y, --yes", "Don't prompt to install"), opt("-p, --package", "Package to install").arg(arg("package"))])
        .arg(arg("command").command())
}

pub fn pip(name: &str) -> Subcommand {
    let installed = || arg("package").variadic().generator(gens::pip_installed());
    cmd(name, "The Python package installer")
        .opts([opt("-V, --version", "Show version"), opt("-q, --quiet", "Less output").persistent(), opt("-v, --verbose", "More output").persistent()])
        .subs([
            cmd("install", "Install packages")
                .opts([
                    opt("-r, --requirement", "Install from a requirements file").repeatable().arg(arg("file").files()),
                    opt("-e, --editable", "Install in editable mode").arg(arg("path").folders()),
                    opt("-U, --upgrade", "Upgrade packages"),
                    opt("--user", "Install to the user site"),
                    opt("--pre", "Include pre-release versions"),
                    opt("--no-deps", "Don't install dependencies"),
                    opt("--force-reinstall", "Reinstall all packages"),
                    opt("--break-system-packages", "Allow modifying an externally-managed environment"),
                    opt("-t, --target", "Install into this directory").arg(arg("dir").folders()),
                    opt("-i, --index-url", "Base URL of the package index").arg(arg("url")),
                ])
                .arg(arg("package").optional().variadic().files()),
            cmd("uninstall", "Uninstall packages").opts([opt("-y, --yes", "Don't ask for confirmation"), opt("-r, --requirement", "Uninstall from a requirements file").arg(arg("file").files())]).arg(installed()),
            cmd("list", "List installed packages").opts([opt("-o, --outdated", "List outdated packages"), opt("-u, --uptodate", "List up-to-date packages"), opt("--user", "User site only"), opt("--format", "Output format").arg(arg("fmt").suggest(&["columns", "freeze", "json"]))]),
            cmd("show", "Show information about installed packages").opt(opt("-f, --files", "Show installed files")).arg(installed()),
            cmd("freeze", "Output installed packages in requirements format").opt(opt("--all", "Include pip, setuptools, wheel")),
            cmd("download", "Download packages").opt(opt("-d, --dest", "Directory").arg(arg("dir").folders())).arg(arg("package").variadic()),
            cmd("wheel", "Build wheels").opt(opt("-w, --wheel-dir", "Directory").arg(arg("dir").folders())).arg(arg("package").variadic().files()),
            cmd("check", "Verify installed packages have compatible dependencies"),
            cmd("cache", "Inspect and manage pip's cache").subs([cmd("dir", "Show the cache directory"), cmd("info", "Show cache information"), cmd("list", "List cached wheels"), cmd("purge", "Remove all items")]),
            cmd("config", "Manage configuration").subs([cmd("list", "List values"), cmd("edit", "Edit the config file"), cmd("get", "Get a value").arg(arg("name")), cmd("set", "Set a value").arg(arg("name")).arg(arg("value")), cmd("unset", "Unset a value").arg(arg("name"))]),
            cmd("search", "Search PyPI").arg(arg("query")),
            cmd("inspect", "Inspect the Python environment"),
            cmd("help", "Show help for commands").arg(arg("command").optional()),
        ])
}
