use super::gens;
use crate::spec::{Arg, Opt, Subcommand, arg, cmd, opt};

fn container() -> Arg {
    arg("container").generator(gens::docker_containers(true))
}

fn running() -> Arg {
    arg("container").generator(gens::docker_containers(false))
}

fn image() -> Arg {
    arg("image").generator(gens::docker_images())
}

/// Local images first, then Docker Hub.
fn remote_image() -> Arg {
    image().generator(gens::docker_hub())
}

fn run_opts() -> Vec<Opt> {
    vec![
        opt("-d, --detach", "Run in the background"),
        opt("-i, --interactive", "Keep STDIN open"),
        opt("-t, --tty", "Allocate a pseudo-TTY"),
        opt("-it", "Interactive with a TTY"),
        opt("--rm", "Remove the container when it exits"),
        opt("--name", "Container name").arg(arg("name")),
        opt("-p, --publish", "Publish a port (host:container)").repeatable().arg(arg("port")),
        opt("-P, --publish-all", "Publish all exposed ports"),
        opt("-v, --volume", "Bind mount a volume (src:dst)").repeatable().arg(arg("volume").files()),
        opt("--mount", "Attach a filesystem mount").repeatable().arg(arg("mount")),
        opt("-e, --env", "Set an environment variable").repeatable().arg(arg("VAR=value")),
        opt("--env-file", "Read environment variables from a file").arg(arg("file").files()),
        opt("-w, --workdir", "Working directory inside the container").arg(arg("dir")),
        opt("-u, --user", "Username or UID").arg(arg("user")),
        opt("--network", "Connect to a network").arg(arg("network").generator(gens::docker_list("docker:networks", "network")).suggest(&["host", "bridge", "none"])),
        opt("--entrypoint", "Override the entrypoint").arg(arg("command")),
        opt("--restart", "Restart policy").arg(arg("policy").suggest(&["no", "always", "on-failure", "unless-stopped"])),
        opt("--platform", "Target platform").arg(arg("platform").suggest(&["linux/amd64", "linux/arm64"])),
        opt("--gpus", "GPU devices to add").arg(arg("gpus").suggest(&["all"])),
        opt("--privileged", "Give extended privileges"),
        opt("--init", "Run an init inside the container"),
        opt("--add-host", "Add a host-to-IP mapping").arg(arg("host:ip")),
        opt("-m, --memory", "Memory limit").arg(arg("bytes")),
        opt("--cpus", "Number of CPUs").arg(arg("cpus")),
        opt("-l, --label", "Set metadata").arg(arg("label")),
        opt("--pull", "Pull policy").arg(arg("policy").suggest(&["always", "missing", "never"])),
    ]
}

fn compose() -> Subcommand {
    let svc = || arg("service").optional().variadic().generator(gens::compose_services());
    cmd("compose", "Define and run multi-container applications")
        .opts([
            opt("-f, --file", "Compose file").persistent().repeatable().arg(arg("file").files()),
            opt("-p, --project-name", "Project name").persistent().arg(arg("name")),
            opt("--profile", "Enable a profile").persistent().repeatable().arg(arg("profile")),
            opt("--env-file", "Environment file").persistent().arg(arg("file").files()),
        ])
        .subs([
            cmd("up", "Create and start containers")
                .opts([
                    opt("-d, --detach", "Run in the background"),
                    opt("--build", "Build images before starting"),
                    opt("--force-recreate", "Recreate containers"),
                    opt("--remove-orphans", "Remove orphan containers"),
                    opt("--no-deps", "Don't start linked services"),
                    opt("-w, --watch", "Watch source code and rebuild"),
                    opt("--wait", "Wait for services to be healthy"),
                ])
                .arg(svc()),
            cmd("down", "Stop and remove containers, networks")
                .opts([opt("-v, --volumes", "Remove volumes"), opt("--rmi", "Remove images").arg(arg("type").suggest(&["all", "local"])), opt("--remove-orphans", "Remove orphan containers")]),
            cmd("logs", "View output from containers")
                .opts([opt("-f, --follow", "Follow log output"), opt("-n, --tail", "Number of lines").arg(arg("n")), opt("-t, --timestamps", "Show timestamps")])
                .arg(svc()),
            cmd("ps", "List containers").opts([opt("-a, --all", "Show stopped containers"), opt("-q, --quiet", "Only IDs")]).arg(svc()),
            cmd("build", "Build or rebuild services").opts([opt("--no-cache", "Don't use cache"), opt("--pull", "Pull newer images")]).arg(svc()),
            cmd("pull", "Pull service images").arg(svc()),
            cmd("push", "Push service images").arg(svc()),
            cmd("restart", "Restart services").arg(svc()),
            cmd("stop", "Stop services").arg(svc()),
            cmd("start", "Start services").arg(svc()),
            cmd("rm", "Remove stopped containers").opts([opt("-f, --force", "Don't ask"), opt("-s, --stop", "Stop first")]).arg(svc()),
            cmd("exec", "Execute a command in a running container")
                .opts([opt("-d, --detach", "Detached mode"), opt("-T", "Disable TTY"), opt("-u, --user", "User").arg(arg("user")), opt("-w, --workdir", "Working directory").arg(arg("dir"))])
                .arg(arg("service").generator(gens::compose_services()))
                .arg(arg("command").command()),
            cmd("run", "Run a one-off command on a service")
                .opts([opt("--rm", "Remove the container afterwards"), opt("-d, --detach", "Detached mode"), opt("-e", "Environment variable").arg(arg("VAR=value"))])
                .arg(arg("service").generator(gens::compose_services()))
                .arg(arg("command").optional().command()),
            cmd("config", "Render the compose file").opts([opt("--services", "Print service names"), opt("-q, --quiet", "Only validate")]),
            cmd("top", "Display running processes").arg(svc()),
            cmd("watch", "Watch build context and rebuild").arg(svc()),
        ])
}

pub fn spec() -> Subcommand {
    cmd("docker", "Container runtime")
        .opts([
            opt("--context, -c", "Context to use").persistent().arg(arg("context")),
            opt("-H, --host", "Daemon socket").persistent().arg(arg("host")),
            opt("-v, --version", "Print version"),
        ])
        .subs([
            cmd("run", "Create and run a new container from an image").opts(run_opts()).arg(remote_image()).arg(arg("command").optional().command()),
            cmd("exec", "Execute a command in a running container")
                .opts([
                    opt("-i, --interactive", "Keep STDIN open"),
                    opt("-t, --tty", "Allocate a pseudo-TTY"),
                    opt("-it", "Interactive with a TTY"),
                    opt("-d, --detach", "Run in the background"),
                    opt("-u, --user", "Username or UID").arg(arg("user")),
                    opt("-w, --workdir", "Working directory").arg(arg("dir")),
                    opt("-e, --env", "Environment variable").repeatable().arg(arg("VAR=value")),
                ])
                .arg(running())
                .arg(arg("command").command()),
            cmd("ps", "List containers").opts([
                opt("-a, --all", "Show all containers"),
                opt("-q, --quiet", "Only display IDs"),
                opt("-s, --size", "Display total file sizes"),
                opt("-f, --filter", "Filter output").arg(arg("filter")),
                opt("--format", "Format output").arg(arg("format").suggest(&["table", "json"])),
                opt("-n, --last", "Show n last created containers").arg(arg("n")),
            ]),
            cmd("images", "List images").opts([opt("-a, --all", "Show all images"), opt("-q, --quiet", "Only IDs"), opt("--digests", "Show digests")]),
            cmd("build", "Build an image from a Dockerfile")
                .opts([
                    opt("-t, --tag", "Name and tag").repeatable().arg(arg("name:tag")),
                    opt("-f, --file", "Dockerfile").arg(arg("file").files()),
                    opt("--build-arg", "Build-time variable").repeatable().arg(arg("VAR=value")),
                    opt("--no-cache", "Do not use cache"),
                    opt("--pull", "Always pull newer base images"),
                    opt("--target", "Target build stage").arg(arg("stage")),
                    opt("--platform", "Target platform").arg(arg("platform").suggest(&["linux/amd64", "linux/arm64"])),
                    opt("--progress", "Progress output").arg(arg("type").suggest(&["auto", "plain", "tty"])),
                    opt("-q, --quiet", "Suppress output"),
                ])
                .arg(arg("context").folders()),
            cmd("pull", "Download an image from a registry").opts([opt("-a, --all-tags", "Download all tags"), opt("--platform", "Platform").arg(arg("platform"))]).arg(remote_image()),
            cmd("push", "Upload an image to a registry").arg(image()),
            cmd("logs", "Fetch the logs of a container")
                .opts([
                    opt("-f, --follow", "Follow log output"),
                    opt("-n, --tail", "Number of lines to show").arg(arg("n").suggest(&["100", "all"])),
                    opt("-t, --timestamps", "Show timestamps"),
                    opt("--since", "Logs since a timestamp or duration").arg(arg("time")),
                ])
                .arg(container()),
            cmd("stop", "Stop running containers").opt(opt("-t, --time", "Seconds to wait").arg(arg("seconds"))).arg(running().variadic()),
            cmd("start", "Start stopped containers").opts([opt("-a, --attach", "Attach output"), opt("-i, --interactive", "Attach STDIN")]).arg(container().variadic()),
            cmd("restart", "Restart containers").arg(container().variadic()),
            cmd("kill", "Kill running containers").opt(opt("-s, --signal", "Signal").arg(arg("signal"))).arg(running().variadic()),
            cmd("rm", "Remove containers").opts([opt("-f, --force", "Force removal"), opt("-v, --volumes", "Remove anonymous volumes")]).arg(container().variadic()),
            cmd("rmi", "Remove images").opt(opt("-f, --force", "Force removal")).arg(image().variadic()),
            cmd("inspect", "Low-level information on objects").opt(opt("-f, --format", "Format").arg(arg("template"))).arg(container().generator(gens::docker_images()).variadic()),
            cmd("tag", "Create a tag that refers to an image").arg(image()).arg(arg("target")),
            cmd("cp", "Copy files between a container and the filesystem").arg(arg("src").files().generator(gens::docker_containers(true))).arg(arg("dest").files()),
            cmd("stats", "Live resource usage statistics").opts([opt("-a, --all", "All containers"), opt("--no-stream", "Print once")]).arg(running().optional().variadic()),
            cmd("top", "Running processes of a container").arg(running()),
            cmd("attach", "Attach to a running container").arg(running()),
            cmd("login", "Log in to a registry").opts([opt("-u, --username", "Username").arg(arg("user")), opt("--password-stdin", "Read the password from stdin")]).arg(arg("server").optional()),
            cmd("logout", "Log out from a registry").arg(arg("server").optional()),
            cmd("save", "Save images to a tar archive").opt(opt("-o, --output", "Output file").arg(arg("file").files())).arg(image().variadic()),
            cmd("load", "Load an image from a tar archive").opt(opt("-i, --input", "Input file").arg(arg("file").files())),
            cmd("search", "Search Docker Hub for images").arg(arg("term")),
            cmd("version", "Show version information"),
            cmd("info", "Display system-wide information"),
            compose(),
            cmd("container", "Manage containers").subs([
                cmd("ls", "List containers").opt(opt("-a, --all", "Show all")),
                cmd("rm", "Remove containers").opt(opt("-f, --force", "Force")).arg(container().variadic()),
                cmd("prune", "Remove all stopped containers").opt(opt("-f, --force", "Don't prompt")),
                cmd("inspect", "Inspect containers").arg(container().variadic()),
                cmd("stop", "Stop containers").arg(running().variadic()),
            ]),
            cmd("image", "Manage images").subs([
                cmd("ls", "List images").opt(opt("-a, --all", "Show all")),
                cmd("rm", "Remove images").opt(opt("-f, --force", "Force")).arg(image().variadic()),
                cmd("prune", "Remove unused images").opts([opt("-a, --all", "All unused images"), opt("-f, --force", "Don't prompt")]),
                cmd("inspect", "Inspect images").arg(image().variadic()),
                cmd("history", "Show the history of an image").arg(image()),
            ]),
            cmd("network", "Manage networks").subs([
                cmd("ls", "List networks"),
                cmd("create", "Create a network").opt(opt("-d, --driver", "Driver").arg(arg("driver").suggest(&["bridge", "overlay", "host", "macvlan"]))).arg(arg("name")),
                cmd("rm", "Remove networks").arg(arg("network").variadic().generator(gens::docker_list("docker:networks", "network"))),
                cmd("inspect", "Inspect networks").arg(arg("network").variadic().generator(gens::docker_list("docker:networks", "network"))),
                cmd("connect", "Connect a container to a network").arg(arg("network").generator(gens::docker_list("docker:networks", "network"))).arg(container()),
                cmd("disconnect", "Disconnect a container from a network").arg(arg("network").generator(gens::docker_list("docker:networks", "network"))).arg(container()),
                cmd("prune", "Remove unused networks").opt(opt("-f, --force", "Don't prompt")),
            ]),
            cmd("volume", "Manage volumes").subs([
                cmd("ls", "List volumes"),
                cmd("create", "Create a volume").arg(arg("name").optional()),
                cmd("rm", "Remove volumes").opt(opt("-f, --force", "Force")).arg(arg("volume").variadic().generator(gens::docker_list("docker:volumes", "volume"))),
                cmd("inspect", "Inspect volumes").arg(arg("volume").variadic().generator(gens::docker_list("docker:volumes", "volume"))),
                cmd("prune", "Remove unused volumes").opts([opt("-a, --all", "All unused volumes"), opt("-f, --force", "Don't prompt")]),
            ]),
            cmd("system", "Manage Docker").subs([
                cmd("df", "Show disk usage").opt(opt("-v, --verbose", "Detailed")),
                cmd("prune", "Remove unused data").opts([opt("-a, --all", "All unused images"), opt("--volumes", "Prune volumes"), opt("-f, --force", "Don't prompt")]),
                cmd("info", "Display system-wide information"),
                cmd("events", "Get real time events"),
            ]),
            cmd("context", "Manage contexts").subs([
                cmd("ls", "List contexts"),
                cmd("use", "Set the current context").arg(arg("context").generator(crate::spec::gen_fn("docker:contexts", |cx| {
                    cx.lines(&["docker", "context", "ls", "--format", "{{.Name}}"]).into_iter().map(crate::spec::sug).collect()
                }))),
                cmd("create", "Create a context").arg(arg("name")),
                cmd("rm", "Remove contexts").arg(arg("context").variadic()),
            ]),
            cmd("buildx", "Docker Buildx").help_from(&["docker", "buildx"]),
        ])
}
