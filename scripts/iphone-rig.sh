#!/bin/sh
# ONE COPY OF THE IPHONE APP, ON ITS OWN SIMULATOR DEVICE, FOR ONE COMMAND, WITH NO WINDOW. NEVER TWO.
#
#   WHOOSH_RIG_HIDDEN=1 sh scripts/iphone-rig.sh <built iPhone .app for the simulator> -- <command> [args...]
#   WHOOSH_RIG_HIDDEN=1 sh scripts/iphone-rig.sh --with-mac <built Mac .app> <built iPhone .app> -- <command> [args...]
#   WHOOSH_RIG_HIDDEN=1 sh scripts/iphone-rig.sh --with-mac --shared-server <built Mac .app> <built iPhone .app> -- <command> [args...]
#   sh scripts/iphone-rig.sh --selftest
#
# The model is scripts/launch-one.sh (the Mac app's hidden launcher); the owner's rule is the same
# (2026-11-08 16:30: "on all tests, EVER, i want you to stop the preview opening EVER"):
#   · it uses ONLY `xcrun simctl`. It never runs `open`, never starts Simulator.app, never starts Xcode. A booted
#     simulator device has no window unless Simulator.app is open; so this script REFUSES (exit 6, "REFUSED:
#     Simulator.app is open") when a process named Simulator runs, before it starts, after the device booted, and
#     after the command ended (a Simulator that appeared during the run is reported with exit 6 as well);
#   · it needs WHOOSH_RIG_HIDDEN=1 (exit 6 otherwise), and gives the app WHOOSH_RIG_HIDDEN=1 too;
#   · it creates ONE device of its own, named $RIG_DEVICE_NAME, on the iOS 26.5 runtime, and touches no other device
#     (it finds its device by that exact name; the NexusTV devices and every other device are never listed for action);
#   · the app gets a private control-socket folder $D. The app (Sources/ControlSocket/ControlServer.swift:7-8,15) binds
#     $WHOOSH_SOCKET_DIR/whoosh.sock, and a simulator app shares the host's file system, so the host reaches the same
#     file. The folder is /tmp/wrig-<pid> (RIG_ROOT, default /tmp): a unix socket path is limited to 104 bytes
#     (ControlServer.swift:26 refuses a longer one, ENAMETOOLONG) and the device's own data folder under
#     ~/Library/Developer/CoreSimulator/Devices/<udid>/data/... is about 150 bytes before the file name;
#   · the test engine by default: the app is launched with `--fixture-core` (Sources/CoreBridge/CoreHost.swift:16,
#     the same flag launch-one.sh passes the Mac copy; RIG_APP_ARGS overrides, even to empty). Nothing leaves this Mac;
#   · one phone rig at a time: a mkdir lock that holds the holder's pid (a dead holder's lock is cleared);
#   · every simctl call runs under a time limit (perl alarm; this Mac has no `timeout`).
#
# The command runs with: D, WHOOSH_SOCKET_DIR, WHOOSH_IPHONE_SOCKET_DIR (all three = the phone's socket folder),
# RIG_UDID, RIG_PID (the app's pid on the host: a simulated process is a host process), WHOOSH_RIG_HIDDEN=1.
# With --with-mac the Mac hidden copy runs first (through scripts/launch-one.sh, unchanged), the phone rig runs inside
# it, and then WHOOSH_SOCKET_DIR stays the Mac copy's folder (launch-one's), WHOOSH_IPHONE_SOCKET_DIR is the phone's, and
# WHOOSH_MAC_SOCKET_DIR (read by journeys/J7.py:61) is the Mac copy's folder too.
# With --shared-server (only with --with-mac; item f3cafb91) BOTH copies run the REAL engine against ONE loopback IMAP and SMTP test server, so an archive or a reply on the
# phone can reach the Mac: the rig starts checks/realcore_mailfake.py (`--serve --act`, 127.0.0.1 only, one seeded account, mail the Mac and the phone both read), launches the
# Mac copy with `--rig-hidden` and the phone without `--fixture-core`, hands both WHOOSH_CORE_PLAIN_LOOPBACK=1 and WHOOSH_CORE_CHECK_SERVERS=<imap>:<smtp> (the engine's
# rig-only seam, core/crates/engine/src/rust_core.rs: loopback ports only), exports WHOOSH_SHARED_SERVER=<imap>:<smtp> to the command, and stops the server by its pid at the end.
# Every other run keeps the test engine. Nothing leaves this Mac: the server has no other address, the copies have no real account, the Keychain items are the rig's own.
# With --with-mac the working folder becomes the repository root (launch-one.sh reads docs/pipeline.env from there).
#
# Exit: the command's exit code, or
#   2   usage, a missing app, runtime or device type            3   boot, install, launch failed, or NO SOCKET
#   4   DIED: the app was gone when the command ended           6   REFUSED (hidden mode not set, or Simulator.app is open)
#   75  BUSY: the lock stayed held for RIG_BUSY_WAIT seconds
# Every run prints:   instances: phone-rig=N simulator-app=N     (N counted after the shutdown)
#
# Config (environment):
#   RIG_DEVICE_NAME  default "Whoosh rig iPhone 17 Pro"      RIG_DEVICE_TYPE  default com.apple.CoreSimulator.SimDeviceType.iPhone-17-Pro
#   RIG_RUNTIME_ID   default com.apple.CoreSimulator.SimRuntime.iOS-26-5
#   RIG_LEAVE_WELCOME=1 (leave the first-launch welcome before the command) · RIG_LAUNCH_TRIES (5) · RIG_LAUNCH_PAUSE (5 s) · the retry installs the app again
#   (a fresh container: the app is uninstalled after the boot and before the install, every run)
#   RIG_ROOT (/tmp) · RIG_APP_ARGS (--fixture-core) · RIG_WAIT (60 s for the socket) · RIG_BUSY_WAIT (120 s)
#   RIG_LAUNCH_ONE   the Mac launcher (default scripts/launch-one.sh next to this file; the selftest points it at a fake)
#   logs             $RIG_ROOT/wrig-simctl.log (every simctl call and answer) and $RIG_ROOT/wrig-app.log (the app's output); kept
set -u

SELF="$0"
case "$SELF" in /*) ;; *) SELF="$(cd "$(dirname "$0")" && pwd -P)/$(basename "$0")" ;; esac
REPO="$(cd "$(dirname "$SELF")/.." && pwd -P)"

say() { printf '%s\n' "$*"; }

# ---------------------------------------------------------------------------------------------- the selftest
selftest() {
  # a SHORT folder: the fake app binds a unix socket under it (limit 104 bytes), and $TMPDIR here is already about 50
  T_ROOT=$(mktemp -d "${RIG_SELFTEST_ROOT:-/tmp}/wrigst.XXXXXX") || { say "selftest: no temp folder"; return 2; }
  FK="$T_ROOT/fake"; mkdir -p "$FK/bin" "$T_ROOT/root" "$T_ROOT/app/Whoosh.app" "$T_ROOT/mac/Whoosh.app/Contents"
  printf '%s\n' '<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>io.nexus.whoosh</string></dict></plist>' > "$T_ROOT/app/Whoosh.app/Info.plist"
  # fake xcrun: records its arguments; answers like simctl (list runtimes, list devices, create, boot, install, launch, terminate, shutdown)
  cat > "$FK/bin/xcrun" <<'FAKEXCRUN'
#!/bin/sh
echo "xcrun $*" >> "$FAKE_LOG"
[ "$1" = simctl ] || { echo "fake xcrun: not simctl" >&2; exit 9; }
v="$2"; shift 2
case "$v" in
  list)
    case "$1" in
      runtimes) echo "== Runtimes =="; echo "iOS 26.5 (26.5 - 23F77) - com.apple.CoreSimulator.SimRuntime.iOS-26-5" ;;
      devices) echo "== Devices =="; echo "-- tvOS 26.5 --"; cat "$FAKE_DIR/devices" ;;
    esac ;;
  create) udid=AAAAAAAA-1111-2222-3333-444444444444; echo "    $1 ($udid) (Shutdown) " >> "$FAKE_DIR/devices"; echo "$udid" ;;
  boot) grep -q "($1)" "$FAKE_DIR/devices" || { echo "no such device" >&2; exit 1; }
        if [ "${FAKE_FAIL:-}" = boot ]; then echo "boot failed" >&2; exit 1; fi
        sed "s/($1) (Shutdown)/($1) (Booted)/" "$FAKE_DIR/devices" > "$FAKE_DIR/d2" && mv "$FAKE_DIR/d2" "$FAKE_DIR/devices" ;;
  install) if [ -n "${FAKE_SLOW_ONCE:-}" ] && [ ! -e "$FAKE_DIR/slow-install" ]; then : > "$FAKE_DIR/slow-install"; exit 142; fi
           if [ "${FAKE_FAIL:-}" = install ]; then echo "install failed" >&2; exit 1; fi ;;
  launch)
    if [ -n "${FAKE_LAUNCH_FAIL_ONCE:-}" ] && [ ! -e "$FAKE_DIR/launched-once" ]; then
      : > "$FAKE_DIR/launched-once"; echo 'The request was denied by service delegate (SBMainWorkspace) for reason: NotFound ("Application \"io.nexus.whoosh\" is unknown to FrontBoard").' >&2; exit 1
    fi
    echo "env: $(env | grep '^SIMCTL_CHILD_' | sed 's/=.*//' | sort | tr '\n' ' ')" >> "$FAKE_LOG"
    echo "env-socket: ${SIMCTL_CHILD_WHOOSH_SOCKET_DIR:-}" >> "$FAKE_LOG"
    echo "env-hidden: ${SIMCTL_CHILD_WHOOSH_RIG_HIDDEN:-}" >> "$FAKE_LOG"
    echo "env-servers: ${SIMCTL_CHILD_WHOOSH_CORE_CHECK_SERVERS:-} loopback=${SIMCTL_CHILD_WHOOSH_CORE_PLAIN_LOOPBACK:-}" >> "$FAKE_LOG"
    python3 -c 'import socket,sys; s=socket.socket(socket.AF_UNIX); s.bind(sys.argv[1])' "${SIMCTL_CHILD_WHOOSH_SOCKET_DIR}/whoosh.sock" || exit 1
    echo "io.nexus.whoosh: $FAKE_APP_PID" ;;
  terminate) : ;;
  uninstall) : ;;
  bootstatus) : ;;
  shutdown) sed "s/($1) (Booted)/($1) (Shutdown)/" "$FAKE_DIR/devices" > "$FAKE_DIR/d2" && mv "$FAKE_DIR/d2" "$FAKE_DIR/devices" ;;
  *) echo "fake xcrun: unknown verb $v" >&2; exit 9 ;;
esac
FAKEXCRUN
  printf '%s\n' '#!/bin/sh' 'echo "ps $*" >> "$FAKE_LOG"' 'cat "$FAKE_DIR/ps" 2>&1' > "$FK/bin/ps"
  printf '%s\n' '#!/bin/sh' 'echo "open $*" >> "$FAKE_LOG"' 'echo "open $*" >> "$FAKE_DIR/open.calls"' > "$FK/bin/open"
  # fake launch-one: same shape (APP_ARGS words, then the command), exports the Mac copy's folder the way launch-one does
  cat > "$FK/launch-one.sh" <<'FAKEL1'
#!/bin/sh
echo "launch-one $*" >> "$FAKE_LOG"
echo "launch-env: ${LAUNCH_ENV:-}" >> "$FAKE_LOG"
bundle="$1"; shift 2
while [ $# -gt 0 ]; do case "$1" in --*) shift ;; *) break ;; esac; done
mkdir -p "$FAKE_DIR/macrun"; WHOOSH_SOCKET_DIR="$FAKE_DIR/macrun"; D="$FAKE_DIR/macrun"; export WHOOSH_SOCKET_DIR D
exec "$@"
FAKEL1
  cat > "$FK/mailfake.py" <<'FAKESRV'
import os, sys, time
a = sys.argv
d = os.environ["FAKE_DIR"]
open(d + "/srv.args", "w").write(" ".join(a[1:]))
if os.environ.get("FAKE_SRV_FAIL"):
    sys.exit(1)
open(d + "/srv.pid", "w").write(str(os.getpid()))
open(a[a.index("--ports-file") + 1], "w").write("4101:4102\n")
time.sleep(600)
FAKESRV
  chmod +x "$FK/bin/xcrun" "$FK/bin/ps" "$FK/bin/open" "$FK/launch-one.sh"
  n=0; bad=0
  ok() { n=$((n+1)); if [ "$2" = 1 ]; then say "PASS $1"; else say "FAIL $1"; bad=$((bad+1)); fi; }
  reset() {
    : > "$FK/log"; rm -f "$FK/open.calls" "$FK/cmd.out" "$FK/ps"; : > "$FK/ps"
    printf '%s\n' '-- tvOS 26.5 --' '    NexusTV-1080p (3C0A0020-7494-4149-8764-091FB4E0FB84) (Shutdown) ' > "$FK/devices"
    printf '    a-user-ios (BBBBBBBB-1111-2222-3333-444444444444) (Booted) \n' >> "$FK/devices"
    rm -rf "$T_ROOT/root" "$FK/macrun"; mkdir -p "$T_ROOT/root"
  }
  run() {  # run <hidden:1|0> <args...>; output to $FK/out, status to $FK/rc
    _h="$1"; shift
    if [ "$_h" = 1 ]; then _hv=1; else _hv=""; fi
    ( PATH="$FK/bin:$PATH" FAKE_LOG="$FK/log" FAKE_DIR="$FK" FAKE_APP_PID=$$ RIG_ROOT="$T_ROOT/root" RIG_BUSY_WAIT=${T_BUSY:-0} RIG_WAIT=3 \
      RIG_LAUNCH_ONE="$FK/launch-one.sh" RIG_MAILFAKE="$FK/mailfake.py" WHOOSH_RIG_HIDDEN="$_hv" sh "$SELF" "$@" ) > "$FK/out" 2>&1
    echo $? > "$FK/rc"
  }
  verbs() { grep '^xcrun simctl ' "$FK/log" | awk '{print $3}' | grep -E '^(boot|install|launch|terminate|shutdown)$' | tr '\n' ' '; }
  APP="$T_ROOT/app/Whoosh.app"
  CMDOK='sh -c "echo COMMAND >> \"$FAKE_LOG\"; env | grep -E \"^(D|WHOOSH_SOCKET_DIR|WHOOSH_IPHONE_SOCKET_DIR|RIG_UDID|RIG_PID)=\" | sort > \"$FAKE_DIR/cmd.out\""'

  reset; run 0 "$APP" -- true
  [ "$(cat "$FK/rc")" = 6 ] && ! grep -q 'xcrun' "$FK/log" && grep -q 'WHOOSH_RIG_HIDDEN=1 is required' "$FK/out"; ok "refusal-without-WHOOSH_RIG_HIDDEN (exit 6, no simctl call)" $([ $? = 0 ] && echo 1 || echo 0)

  reset; echo "  501 /Applications/Xcode.app/Contents/Developer/Applications/Simulator.app/Contents/MacOS/Simulator" > "$FK/ps"
  run 1 "$APP" -- true
  [ "$(cat "$FK/rc")" = 6 ] && grep -q 'REFUSED: Simulator.app is open' "$FK/out" && ! grep -q 'simctl boot' "$FK/log"; ok "refusal-when-Simulator-process-is-reported (exit 6, no boot)" $([ $? = 0 ] && echo 1 || echo 0)

  reset; run 1 "$APP" -- sh -c 'echo COMMAND >> "$FAKE_LOG"'
  [ "$(cat "$FK/rc")" = 0 ] && [ "$(verbs)" = "boot install launch terminate shutdown " ] && grep -n '' "$FK/log" | awk -F: '/xcrun simctl launch/{l=$1} /^[0-9]+:COMMAND/{c=$1} /xcrun simctl terminate/{t=$1} END{exit !(l<c && c<t)}'
  ok "order-boot-install-launch-command-terminate-shutdown" $([ $? = 0 ] && echo 1 || echo 0)

  reset; run 1 "$APP" -- sh -c 'echo COMMAND >> "$FAKE_LOG"; exit 7'
  [ "$(cat "$FK/rc")" = 7 ] && [ "$(verbs)" = "boot install launch terminate shutdown " ] && grep -q '(AAAAAAAA-1111-2222-3333-444444444444) (Shutdown)' "$FK/devices"
  ok "device-shut-down-when-the-command-fails (exit 7 kept, device Shutdown)" $([ $? = 0 ] && echo 1 || echo 0)

  reset; ( export FAKE_FAIL=install; run 1 "$APP" -- sh -c 'echo COMMAND >> "$FAKE_LOG"' )
  [ "$(cat "$FK/rc")" = 3 ] && ! grep -q '^COMMAND' "$FK/log" && grep -q 'simctl shutdown' "$FK/log" && [ ! -e "$T_ROOT/root/wrig.lock" ]
  ok "device-shut-down-when-install-fails (exit 3, command not run, lock gone)" $([ $? = 0 ] && echo 1 || echo 0)

  reset; run 1 "$APP" -- true
  [ "$(cat "$FK/rc")" = 0 ] && grep -n '' "$FK/log" | awk -F: '/xcrun simctl bootstatus/{s=$1} /xcrun simctl uninstall/{u=$1} /xcrun simctl install/{i=$1} /xcrun simctl boot /{b=$1} END{exit !(b<s && s<u && u<i)}'
  ok "fresh-container-the-app-is-uninstalled-after-boot-and-bootstatus-and-before-install" $([ $? = 0 ] && echo 1 || echo 0)

  reset; rm -f "$FK/slow-install"; ( export FAKE_SLOW_ONCE=1; run 1 "$APP" -- true )
  [ "$(cat "$FK/rc")" = 0 ] && [ "$(grep -c 'simctl install' "$FK/log")" = 2 ] && grep -q 'install hit its time limit' "$FK/out"
  ok "install-that-hits-its-time-limit-is-asked-again-once-and-the-run-goes-on" $([ $? = 0 ] && echo 1 || echo 0)

  reset; rm -f "$FK/launched-once"; ( export FAKE_LAUNCH_FAIL_ONCE=1 RIG_LAUNCH_PAUSE=0; run 1 "$APP" -- true )
  [ "$(cat "$FK/rc")" = 0 ] && [ "$(grep -c 'simctl launch' "$FK/log")" = 2 ] && [ "$(grep -c 'simctl install' "$FK/log")" = 2 ] && grep -q 'launch try 1' "$FK/out"
  ok "launch-is-retried-once-on-unknown-to-FrontBoard-and-the-run-goes-on" $([ $? = 0 ] && echo 1 || echo 0)

  reset; rm -f "$FK/launched-once"; ( export FAKE_LAUNCH_FAIL_ONCE=1 RIG_LAUNCH_TRIES=1; run 1 "$APP" -- true )
  [ "$(cat "$FK/rc")" = 3 ] && grep -q 'LAUNCH FAILED after 1 tries' "$FK/out" && grep -q 'simctl shutdown' "$FK/log"
  ok "launch-failure-after-the-tries-is-exit-3-and-the-device-is-shut-down" $([ $? = 0 ] && echo 1 || echo 0)

  reset; run 1 "$APP" -- true; _a=$(grep -c '^leave-welcome:' "$FK/out")
  reset; ( export RIG_LEAVE_WELCOME=1; run 1 "$APP" -- true )
  [ "$_a" = 0 ] && [ "$(cat "$FK/rc")" = 0 ] && grep -q '^leave-welcome: ' "$FK/out"
  ok "leave-welcome-step-runs-only-when-asked (it answers on a socket that serves nothing: not done, and the run goes on)" $([ $? = 0 ] && echo 1 || echo 0)

  reset; run 1 "$APP" -- true
  [ "$(cat "$FK/rc")" = 0 ] && [ ! -e "$FK/open.calls" ] && ! grep -q '^open ' "$FK/log"
  ok "no-call-of-open" $([ $? = 0 ] && echo 1 || echo 0)

  reset; mkdir -p "$T_ROOT/root/wrig.lock" && echo $$ > "$T_ROOT/root/wrig.lock/pid"
  run 1 "$APP" -- true
  [ "$(cat "$FK/rc")" = 75 ] && grep -q 'BUSY' "$FK/out" && ! grep -q 'simctl boot' "$FK/log"
  ok "lock-held-by-a-live-pid-is-BUSY (exit 75)" $([ $? = 0 ] && echo 1 || echo 0)

  reset; mkdir -p "$T_ROOT/root/wrig.lock" && echo 999999 > "$T_ROOT/root/wrig.lock/pid"
  run 1 "$APP" -- true
  [ "$(cat "$FK/rc")" = 0 ] && grep -q 'stale lock of pid 999999 cleared' "$FK/out" && [ ! -e "$T_ROOT/root/wrig.lock" ]
  ok "stale-lock-is-cleared-and-lock-is-released-after-the-run" $([ $? = 0 ] && echo 1 || echo 0)

  reset; run 1 "$APP" -- sh -c 'env | grep -E "^(D|WHOOSH_SOCKET_DIR|WHOOSH_IPHONE_SOCKET_DIR|RIG_UDID|RIG_PID)=" | sort > "$FAKE_DIR/cmd.out"'
  _d=$(sed -n 's/^D=//p' "$FK/cmd.out")
  [ -n "$_d" ] && [ "$(sed -n 's/^WHOOSH_SOCKET_DIR=//p' "$FK/cmd.out")" = "$_d" ] && [ "$(sed -n 's/^WHOOSH_IPHONE_SOCKET_DIR=//p' "$FK/cmd.out")" = "$_d" ] \
    && [ "$(sed -n 's/^RIG_UDID=//p' "$FK/cmd.out")" = AAAAAAAA-1111-2222-3333-444444444444 ] && [ "$(sed -n 's/^RIG_PID=//p' "$FK/cmd.out")" = "$$" ] \
    && grep -q "^env-socket: $_d\$" "$FK/log" && grep -q '^env-hidden: 1$' "$FK/log" && grep -q -- 'simctl launch .* io.nexus.whoosh --fixture-core' "$FK/log" \
    && [ ${#_d} -lt 60 ]
  ok "environment-D-WHOOSH_SOCKET_DIR-WHOOSH_IPHONE_SOCKET_DIR-RIG_UDID-RIG_PID-and-app-gets-socket-folder-and-fixture-core" $([ $? = 0 ] && echo 1 || echo 0)

  reset; run 1 "$APP" -- true
  _d=$(sed -n 's/^socket folder: //p' "$FK/out" | head -1)
  [ -n "$_d" ] && [ ! -e "$_d" ] && [ "$(tail -1 "$FK/out")" = "instances: phone-rig=0 simulator-app=0" ] \
    && sed -n 1,4p "$FK/out" | grep -q '^device: AAAAAAAA' && sed -n 1,4p "$FK/out" | grep -q '^runtime: ' && sed -n 1,4p "$FK/out" | grep -q '^bundle id: io.nexus.whoosh' && sed -n 1,5p "$FK/out" | grep -q '^socket folder: '
  ok "first-lines-name-udid-runtime-bundle-id-socket-folder-and-last-line-is-the-count-line-folder-removed" $([ $? = 0 ] && echo 1 || echo 0)

  reset; run 1 "$APP" -- true; run 1 "$APP" -- true
  [ "$(grep -c 'simctl create' "$FK/log")" = 1 ] && ! grep -E 'simctl (boot|shutdown|terminate|install|launch) .*(3C0A0020|BBBBBBBB)' "$FK/log" > "$FK/touch.out" 2>&1 && grep -q 'BBBBBBBB-1111-2222-3333-444444444444) (Booted)' "$FK/devices"
  ok "only-its-own-device-is-touched-and-created-once (NexusTV and another device untouched)" $([ $? = 0 ] && echo 1 || echo 0)

  reset; run 1 "$APP" -- sh -c 'echo "  501 /x/Simulator.app/Contents/MacOS/Simulator" > "$FAKE_DIR/ps"'
  [ "$(cat "$FK/rc")" = 6 ] && grep -q 'REFUSED: Simulator.app is open' "$FK/out" && [ "$(verbs)" = "boot install launch terminate shutdown " ]
  ok "Simulator-appearing-during-the-run-is-exit-6-after-cleanup" $([ $? = 0 ] && echo 1 || echo 0)

  reset; run 1 --with-mac "$T_ROOT/mac/Whoosh.app" "$APP" -- sh -c 'env | grep -E "^(WHOOSH_SOCKET_DIR|WHOOSH_IPHONE_SOCKET_DIR|WHOOSH_MAC_SOCKET_DIR)=" | sort > "$FAKE_DIR/cmd.out"'
  _m=$(sed -n 's/^WHOOSH_SOCKET_DIR=//p' "$FK/cmd.out"); _p=$(sed -n 's/^WHOOSH_IPHONE_SOCKET_DIR=//p' "$FK/cmd.out")
  [ "$(sed -n 's/^WHOOSH_MAC_SOCKET_DIR=//p' "$FK/cmd.out")" = "$_m" ] && [ "$(cat "$FK/rc")" = 0 ] && [ "$_m" = "$FK/macrun" ] && [ -n "$_p" ] && [ "$_p" != "$_m" ] && grep -q 'launch-one .* -- --fixture-core sh ' "$FK/log" && [ "$(verbs)" = "boot install launch terminate shutdown " ] \
    && grep -q "^env-socket: $_p\$" "$FK/log"
  ok "with-mac-nests-the-phone-rig-in-launch-one (WHOOSH_SOCKET_DIR=Mac folder, WHOOSH_IPHONE_SOCKET_DIR=phone folder)" $([ $? = 0 ] && echo 1 || echo 0)

  reset; rm -f "$FK/srv.args" "$FK/srv.pid"; run 1 --with-mac --shared-server "$T_ROOT/mac/Whoosh.app" "$APP" -- sh -c 'env | grep -E "^(WHOOSH_SOCKET_DIR|WHOOSH_IPHONE_SOCKET_DIR|WHOOSH_SHARED_SERVER)=" | sort > "$FAKE_DIR/cmd.out"'
  _sp=$(cat "$FK/srv.pid" 2>&1)
  [ "$(cat "$FK/rc")" = 0 ] && grep -q -- '--serve --act' "$FK/srv.args" && grep -q 'launch-one .* -- --rig-hidden sh ' "$FK/log" && ! grep 'launch-one' "$FK/log" | grep -q -- '--fixture-core' \
    && grep -q 'launch-env: .*WHOOSH_CORE_PLAIN_LOOPBACK=1 WHOOSH_CORE_CHECK_SERVERS=4101:4102' "$FK/log" && grep -q '^env-servers: 4101:4102 loopback=1$' "$FK/log" \
    && ! grep 'simctl launch' "$FK/log" | grep -q -- '--fixture-core' && grep -q 'simctl launch .* io.nexus.whoosh -AppleLanguages' "$FK/log" \
    && [ "$(sed -n 's/^WHOOSH_SHARED_SERVER=//p' "$FK/cmd.out")" = 4101:4102 ] && grep -q '^shared server: IMAP and SMTP ports 4101:4102 on 127.0.0.1 only' "$FK/out" \
    && ! kill -0 "$_sp" > "$FK/kill.out" 2>&1 && [ "$(tail -2 "$FK/out" | head -1)" != "" ] && grep -q '^instances: mail-server=0$' "$FK/out" && [ -z "$(find "$T_ROOT/root" -maxdepth 1 -name 'wrig-srv-*')" ]
  ok "shared-server-both-copies-on-the-real-engine-server-started-before-and-stopped-by-pid-after (no --fixture-core, loopback env for both, WHOOSH_SHARED_SERVER for the command)" $([ $? = 0 ] && echo 1 || echo 0)

  reset; rm -f "$FK/srv.args"; run 1 --with-mac "$T_ROOT/mac/Whoosh.app" "$APP" -- sh -c 'env | grep -E "^WHOOSH_SHARED_SERVER=" > "$FAKE_DIR/cmd.out"; true'
  [ "$(cat "$FK/rc")" = 0 ] && [ ! -e "$FK/srv.args" ] && [ ! -s "$FK/cmd.out" ] && grep -q 'launch-one .* -- --fixture-core sh ' "$FK/log" && grep -q 'simctl launch .* --fixture-core' "$FK/log"
  ok "without-the-flag-no-server-no-variable-and-the-test-engine-stays" $([ $? = 0 ] && echo 1 || echo 0)

  reset; run 1 --shared-server "$APP" -- true
  [ "$(cat "$FK/rc")" = 2 ] && grep -q 'goes after --with-mac' "$FK/out" && ! grep -q 'xcrun' "$FK/log"
  ok "shared-server-without-with-mac-is-refused-exit-2" $([ $? = 0 ] && echo 1 || echo 0)

  reset; rm -f "$FK/srv.args"; ( export FAKE_SRV_FAIL=1; run 1 --with-mac --shared-server "$T_ROOT/mac/Whoosh.app" "$APP" -- true )
  [ "$(cat "$FK/rc")" = 3 ] && grep -q 'NO SHARED SERVER' "$FK/out" && ! grep -q 'launch-one' "$FK/log" && ! grep -q 'xcrun' "$FK/log"
  ok "a-mail-server-that-does-not-start-is-exit-3-and-nothing-is-launched" $([ $? = 0 ] && echo 1 || echo 0)

  [ -n "${RIG_SELFTEST_KEEP:-}" ] && say "selftest folder kept: $T_ROOT" || rm -rf "$T_ROOT"
  say "selftest: $n cases, $bad failed"
  [ $bad -eq 0 ]
}

if [ "${1:-}" = "--selftest" ]; then selftest; exit $?; fi

# ---------------------------------------------------------------------------------------------- the run
if [ "${WHOOSH_RIG_HIDDEN:-}" != "1" ]; then
  say "iphone-rig: REFUSED: WHOOSH_RIG_HIDDEN=1 is required; no test window may open on the owner's screen (owner rule 2026-11-08 16:30)"
  exit 6
fi

MODE=plain; MAC=""; SHARED=0
case "${1:-}" in
  --with-mac)
    MODE=outer; shift
    if [ "${1:-}" = --shared-server ]; then SHARED=1; shift; fi
    MAC="${1:-}"; PHONE="${2:-}"; [ $# -ge 2 ] && shift 2 || shift $# ;;
  --shared-server) say "REFUSED: --shared-server goes after --with-mac (both copies share the server)"; exit 2 ;;
  --inner-mac) MODE=inner; PHONE="${2:-}"; [ $# -ge 2 ] && shift 2 || shift $# ;;
  *) PHONE="${1:-}"; [ $# -ge 1 ] && shift ;;
esac
if [ "${1:-}" != "--" ] || [ $# -lt 2 ] || [ -z "$PHONE" ]; then
  say "usage: iphone-rig.sh <iPhone .app> -- <command> [args...] | --with-mac [--shared-server] <Mac .app> <iPhone .app> -- <command> [args...] | --selftest"; exit 2
fi
shift

: "${RIG_ROOT:=/tmp}"; RIG_ROOT=${RIG_ROOT%/}
: "${RIG_DEVICE_NAME:=Whoosh rig iPhone 17 Pro}"
: "${RIG_DEVICE_TYPE:=com.apple.CoreSimulator.SimDeviceType.iPhone-17-Pro}"
: "${RIG_RUNTIME_ID:=com.apple.CoreSimulator.SimRuntime.iOS-26-5}"
: "${RIG_WAIT:=60}"; : "${RIG_BUSY_WAIT:=120}"
# The checks expect English (as the Mac hidden copies, which run with AppleLanguages en): the simulator follows the Mac's own language otherwise
# (first run on this Mac: "Copier" where the check expects "Copy").
if [ "$MODE" = inner ] && [ -n "${RIG_SHARED_PORTS:-}" ]; then
  APP_ARGS=${RIG_APP_ARGS--AppleLanguages (en) -AppleLocale en_US}     # the real engine: no --fixture-core
else
  APP_ARGS=${RIG_APP_ARGS---fixture-core -AppleLanguages (en) -AppleLocale en_US}
fi
LAUNCH_ONE=${RIG_LAUNCH_ONE:-$REPO/scripts/launch-one.sh}

# a process named exactly Simulator (the Simulator.app executable); "pid" per line
sim_procs() { ps -axo pid=,comm= | awk '{ p = $2; for (i = 3; i <= NF; i++) p = p " " $i; n = split(p, a, "/"); if (a[n] == "Simulator") print $1 }'; }
sim_count() { sim_procs | wc -l | tr -d ' '; }
refuse_sim() { say "REFUSED: Simulator.app is open (pid $(sim_procs | head -1)); a booted device would show in its window. Quit it, then run again."; }

if [ "$MODE" = outer ]; then
  [ -f "$MAC/Contents/Info.plist" ] || [ -n "${RIG_LAUNCH_ONE:-}" ] || { say "REFUSED: no Mac bundle at $MAC"; exit 2; }
  [ -x "$LAUNCH_ONE" ] || [ -f "$LAUNCH_ONE" ] || { say "REFUSED: no launcher at $LAUNCH_ONE"; exit 2; }
  if [ "$SHARED" != 1 ]; then
    unset RIG_SHARED_PORTS
    cd "$REPO" && exec sh "$LAUNCH_ONE" "$MAC" -- --fixture-core sh "$SELF" --inner-mac "$PHONE" -- "$@"
  fi
  MAILFAKE="${RIG_MAILFAKE:-$REPO/checks/realcore_mailfake.py}"
  [ -f "$MAILFAKE" ] || { say "REFUSED: no mail server script at $MAILFAKE"; exit 2; }
  mkdir -p "$RIG_ROOT" || exit 2
  SRVLOG="$RIG_ROOT/wrig-server.log"; SRVD="$RIG_ROOT/wrig-srv-$$"
  rm -rf "$SRVD"; mkdir "$SRVD" || exit 2
  python3 "$MAILFAKE" --serve --act --messages "${RIG_SHARED_MESSAGES:-6}" --ports-file "$SRVD/ports" > "$SRVLOG" 2>&1 &
  SRV=$!
  stop_srv() { kill "$SRV" >> "$SRVLOG" 2>&1; wait "$SRV" >> "$SRVLOG" 2>&1; rm -rf "$SRVD"; }
  trap 'stop_srv; exit 130' INT TERM
  i=0
  while [ ! -s "$SRVD/ports" ] && [ $i -lt 40 ]; do kill -0 "$SRV" >> "$SRVLOG" 2>&1 || break; sleep 0.5; i=$((i+1)); done
  RIG_SHARED_PORTS=$(tr -d '\n' < "$SRVD/ports" 2>&1)
  case "$RIG_SHARED_PORTS" in
    [0-9]*:[0-9]*) case "$RIG_SHARED_PORTS" in *[!0-9:]*) RIG_SHARED_PORTS="" ;; esac ;;
    *) RIG_SHARED_PORTS="" ;;
  esac
  if [ -z "$RIG_SHARED_PORTS" ]; then
    say "NO SHARED SERVER: the mail server did not publish its ports within 20s (see $SRVLOG)"; tail -5 "$SRVLOG"; stop_srv; exit 3
  fi
  export RIG_SHARED_PORTS
  say "shared server: IMAP and SMTP ports $RIG_SHARED_PORTS on 127.0.0.1 only (pid $SRV, log $SRVLOG)"
  LAUNCH_ENV="${LAUNCH_ENV:-} WHOOSH_CORE_PLAIN_LOOPBACK=1 WHOOSH_CORE_CHECK_SERVERS=$RIG_SHARED_PORTS"; export LAUNCH_ENV
  cd "$REPO" && sh "$LAUNCH_ONE" "$MAC" -- --rig-hidden sh "$SELF" --inner-mac "$PHONE" -- "$@"
  rc=$?
  stop_srv
  kill -0 "$SRV" >> "$SRVLOG" 2>&1 && alive=1 || alive=0
  say "instances: mail-server=$alive"
  exit $rc
fi

[ -f "$PHONE/Info.plist" ] || { say "REFUSED: no iPhone bundle at $PHONE (build it with scripts/build-ios.sh; the .app is under <out>/Build/Products/Debug-iphonesimulator/)"; exit 2; }
BUNDLE_ID=$(plutil -extract CFBundleIdentifier raw -o - "$PHONE/Info.plist" 2>&1) || { say "REFUSED: no CFBundleIdentifier in $PHONE/Info.plist: $BUNDLE_ID"; exit 2; }

if [ "$(sim_count)" != 0 ]; then refuse_sim; say "instances: phone-rig=0 simulator-app=$(sim_count)"; exit 6; fi

LOG="$RIG_ROOT/wrig-simctl.log"; APPLOG="$RIG_ROOT/wrig-app.log"
mkdir -p "$RIG_ROOT" || exit 2
: > "$LOG"; : > "$APPLOG"

# every simctl call: a time limit (seconds), its line and its answer in the log
# a busy Mac makes every simctl call slow, not wrong (phone9: boot, install and launch hit their limit at load 15 to 30, exit 142): the limit is multiplied by
# 1 + the one-minute load per core, rounded down, at most 4 (RIG_LOAD_SCALE=0 turns it off); the factor is printed in the log with each call
load_factor() {
  [ "${RIG_LOAD_SCALE:-1}" = 0 ] && { echo 1; return; }
  _l=$(sysctl -n vm.loadavg 2>&1 | tr -d '{}' | awk '{print $1}'); _c=$(sysctl -n hw.ncpu 2>&1)
  awk -v l="$_l" -v c="$_c" 'BEGIN { f = 1; if (l ~ /^[0-9.]+$/ && c ~ /^[0-9]+$/ && c > 0) f = 1 + int(l / c); if (f > 4) f = 4; print f }'
}
sim() {  # sim <seconds> <simctl args...>
  _t="$1"; shift
  _f=$(load_factor); _t=$((_t * _f))
  [ "$_f" -gt 1 ] && printf '== (time limit x%s for the machine load)\n' "$_f" >> "$LOG"
  printf '== xcrun simctl %s\n' "$*" >> "$LOG"
  perl -e 'alarm shift @ARGV; exec @ARGV' "$_t" xcrun simctl "$@" > "$LOG.out" 2>&1
  _r=$?
  cat "$LOG.out" >> "$LOG"; printf '== exit %s\n' "$_r" >> "$LOG"
  return $_r
}

# ---- the lock: one phone rig at a time
LOCK="$RIG_ROOT/wrig.lock"; w=0
while :; do
  if mkdir "$LOCK" >> "$LOG" 2>&1; then break; fi
  hp=$(cat "$LOCK/pid" 2>&1); case "$hp" in ''|*[!0-9]*) hp="" ;; esac
  if [ -n "$hp" ] && ! kill -0 "$hp" >> "$LOG" 2>&1; then say "stale lock of pid $hp cleared"; rm -rf "$LOCK"; continue; fi
  if [ -z "$hp" ] && [ $w -ge 4 ]; then say "a lock with no holder pid cleared"; rm -rf "$LOCK"; continue; fi
  if [ $w -ge "$RIG_BUSY_WAIT" ]; then say "BUSY: another phone rig holds $LOCK (pid ${hp:-unknown}) after ${w}s"; exit 75; fi
  sleep 2; w=$((w+2))
done
echo $$ > "$LOCK/pid"

D="$RIG_ROOT/wrig-$$"; UDID=""; BOOTED=0; LAUNCHED=0; APP=""; DONE=0
finish() {  # terminate the app, shut the device down, remove the folder, release the lock; once
  [ $DONE -eq 0 ] || return 0
  DONE=1
  if [ $LAUNCHED -eq 1 ]; then sim 30 terminate "$UDID" "$BUNDLE_ID" || say "NOTE: terminate answered $? (see $LOG)"; fi
  if [ $BOOTED -eq 1 ]; then sim 60 shutdown "$UDID" || say "NOTE: shutdown answered $? (see $LOG)"; fi
  rm -rf "$D"
  rm -rf "$LOCK"
}
trap 'finish' EXIT
trap 'finish; exit 130' INT TERM

# ---- scrub anything in the environment that simctl would pass to the app, then the runtime and the device
for v in $(env | awk -F= 'index($1, "SIMCTL_CHILD_") == 1 { print $1 }'); do unset "$v"; done

sim 30 list runtimes || { say "REFUSED: simctl list runtimes failed (see $LOG)"; exit 2; }
RT=$(grep -F "$RIG_RUNTIME_ID" "$LOG.out" | head -1)
[ -n "$RT" ] || { say "REFUSED: runtime $RIG_RUNTIME_ID is not installed (see $LOG)"; exit 2; }
RT_NAME=$(printf '%s' "$RT" | sed 's/ - com\.apple.*//')

find_dev() {  # prints "<udid> <state>" of the device with exactly our name
  sim 30 list devices || return 1
  sed -n "s/^ *$RIG_DEVICE_NAME (\([0-9A-F-]\{36\}\)) (\([A-Za-z ]*\)) *\$/\1 \2/p" "$LOG.out" | head -1
}
DEV=$(find_dev) || { say "REFUSED: simctl list devices failed (see $LOG)"; exit 2; }
if [ -z "$DEV" ]; then
  sim 60 create "$RIG_DEVICE_NAME" "$RIG_DEVICE_TYPE" "$RIG_RUNTIME_ID" || { say "REFUSED: could not create device $RIG_DEVICE_NAME (see $LOG)"; exit 2; }
  UDID=$(tail -1 "$LOG.out" | tr -d ' ')
  STATE=Shutdown
else
  UDID=${DEV%% *}; STATE=${DEV#* }
fi
case "$UDID" in ????????-????-????-????-????????????) ;; *) say "REFUSED: no device id (see $LOG)"; exit 2 ;; esac

mkdir -p "$D" || exit 2
say "device: $UDID ($RIG_DEVICE_NAME, $STATE)"
say "runtime: $RT_NAME ($RIG_RUNTIME_ID)"
say "bundle id: $BUNDLE_ID"
say "socket folder: $D"
say "simctl log: $LOG; app log: $APPLOG"

# a leftover boot of OUR device (a run that was killed) is shut down first; no other device is ever named
if [ "$STATE" = Booted ]; then say "leftover boot of $RIG_DEVICE_NAME: shutting it down first"; sim 60 shutdown "$UDID"; fi

BOOTED=1
sim 120 boot "$UDID"; brc=$?
if [ $brc != 0 ]; then
  if [ $brc = 142 ]; then say "boot hit its time limit; asking again once"; sim 120 boot "$UDID" || { say "BOOT FAILED (see $LOG)"; tail -5 "$LOG"; exit 3; }
  else say "BOOT FAILED (see $LOG)"; tail -5 "$LOG"; exit 3; fi
fi
if [ "$(sim_count)" != 0 ]; then refuse_sim; exit 6; fi
# `simctl boot` returns before the home screen is up; the install and the launch need it (phone5: labelscontactsphone and J4, "unknown to FrontBoard" on all 4 tries)
sim 240 bootstatus "$UDID" || say "NOTE: bootstatus answered $? (see $LOG)"
# a FRESH container every run: the device is reused, so the app's data (a downloaded model, its settings) is removed first; a missing app answers an error, which is not one
sim 60 uninstall "$UDID" "$BUNDLE_ID" || say "NOTE: uninstall answered $? (nothing to remove on a first run; see $LOG)"
sim 120 install "$UDID" "$PHONE"; irc=$?
if [ $irc != 0 ]; then
  if [ $irc = 142 ]; then say "install hit its time limit; asking again once"; sim 120 install "$UDID" "$PHONE" || { say "INSTALL FAILED (see $LOG)"; tail -5 "$LOG"; exit 3; }
  else say "INSTALL FAILED (see $LOG)"; tail -5 "$LOG"; exit 3; fi
fi

export SIMCTL_CHILD_WHOOSH_SOCKET_DIR="$D" SIMCTL_CHILD_WHOOSH_RIG_HIDDEN=1
if [ "$MODE" = inner ] && [ -n "${RIG_SHARED_PORTS:-}" ]; then
  export SIMCTL_CHILD_WHOOSH_CORE_PLAIN_LOOPBACK=1 "SIMCTL_CHILD_WHOOSH_CORE_CHECK_SERVERS=$RIG_SHARED_PORTS"
  say "shared server: the phone runs the real engine against 127.0.0.1 ports $RIG_SHARED_PORTS"
fi
# shellcheck disable=SC2086
# the install is not always known to the home screen at once ("Application ... is unknown to FrontBoard", NotFound: phone4 threadphone, exit 3 with
# no totals): that answer, and only that one, is retried (RIG_LAUNCH_TRIES, default 5, RIG_LAUNCH_PAUSE seconds apart, default 5, with a new install before each try)
tries=0
while :; do
  tries=$((tries+1))
  sim 60 launch --terminate-running-process "--stdout=$APPLOG" "--stderr=$APPLOG" "$UDID" "$BUNDLE_ID" $APP_ARGS && break
  lrc=$?
  if [ $tries -lt "${RIG_LAUNCH_TRIES:-5}" ] && { [ $lrc = 142 ] || grep -q "unknown to FrontBoard\|reason: NotFound" "$LOG.out"; }; then
    say "launch try $tries: the app is not known to the home screen yet; installing it again, then again in ${RIG_LAUNCH_PAUSE:-5}s"; sleep "${RIG_LAUNCH_PAUSE:-5}"
    sim 120 install "$UDID" "$PHONE" || { say "INSTALL FAILED (see $LOG)"; tail -5 "$LOG"; exit 3; }
    continue
  fi
  say "LAUNCH FAILED after $tries tries (see $LOG)"; tail -5 "$LOG"; exit 3
done
LAUNCHED=1
unset SIMCTL_CHILD_WHOOSH_SOCKET_DIR SIMCTL_CHILD_WHOOSH_RIG_HIDDEN SIMCTL_CHILD_WHOOSH_CORE_PLAIN_LOOPBACK SIMCTL_CHILD_WHOOSH_CORE_CHECK_SERVERS
APP=$(tail -1 "$LOG.out" | sed -n 's/^.*: \([0-9][0-9]*\) *$/\1/p')
[ -n "$APP" ] || { say "NO INSTANCE: simctl launch printed no pid (see $LOG)"; exit 3; }

i=0
while [ ! -S "$D/whoosh.sock" ] && [ $i -lt $((RIG_WAIT * 2)) ]; do kill -0 "$APP" >> "$LOG" 2>&1 || break; sleep 0.5; i=$((i+1)); done
if [ ! -S "$D/whoosh.sock" ]; then
  if kill -0 "$APP" >> "$LOG" 2>&1; then say "NO SOCKET: the app (pid $APP) published no whoosh.sock in $((i/2))s"; else say "NO INSTANCE: the app (pid $APP) exited before it published whoosh.sock"; fi
  tail -5 "$APPLOG"; exit 3
fi
say "app pid $APP, socket $D/whoosh.sock"

# RIG_LEAVE_WELCOME=1: a test copy of the iPhone app starts its FIRST launch, on iphone-welcome-accounts (journeys/lib/phonestart.py); a check about the
# inbox, a thread, a sheet or a setting leaves it first, through the welcome flow's own controls (scan, the typed fixture code, Continue, Not now): the same
# function the checks use, so a check that does not call it yet still starts on the inbox. Never set for the checks that judge the welcome flow.
if [ "${RIG_LEAVE_WELCOME:-}" = 1 ] && [ -n "${RIG_SHARED_PORTS:-}" ] && [ "$MODE" = inner ]; then
  say "leave-welcome: not done here: on the real engine the phone has no account yet, the check adds it first and then leaves the welcome (journeys/lib/sharedserver.py)"
elif [ "${RIG_LEAVE_WELCOME:-}" = 1 ]; then
  lw=$(python3 -I -c "
import sys
sys.path.insert(0, sys.argv[1])
from lib.ctl import Ctl
from lib import phonestart
try:
    c = Ctl(sys.argv[2]).connect()
    print('leave-welcome: the app is on', phonestart.leave_welcome(c))
except Exception as e:
    print('leave-welcome: not done (%s)' % e)
" "$REPO/journeys" "$D" 2>&1)
  say "$lw"
fi

export D RIG_UDID="$UDID" RIG_PID="$APP"
if [ "$MODE" = inner ]; then
  export WHOOSH_IPHONE_SOCKET_DIR="$D"          # WHOOSH_SOCKET_DIR stays the Mac copy's folder (launch-one.sh exported it)
  export WHOOSH_MAC_SOCKET_DIR="${WHOOSH_SOCKET_DIR:-}"   # journeys/J7.py:61 reads the Mac copy under this name
  if [ -n "${RIG_SHARED_PORTS:-}" ]; then export WHOOSH_SHARED_SERVER="$RIG_SHARED_PORTS"; else unset WHOOSH_SHARED_SERVER; fi
else
  export WHOOSH_SOCKET_DIR="$D" WHOOSH_IPHONE_SOCKET_DIR="$D"
  unset WHOOSH_SHARED_SERVER
fi
"$@"; rc=$?

died=0; kill -0 "$APP" >> "$LOG" 2>&1 || died=1
finish
sims=$(sim_count)
if [ "$sims" != 0 ]; then refuse_sim; rc=6
elif [ $died -eq 1 ] && [ $rc -eq 0 ]; then say "DIED: the app was gone before the command ended; a pass over a dead app is not a pass"; rc=4; fi
left=$(find_dev | awk '$2 == "Booted"' | wc -l | tr -d ' ')
say "instances: phone-rig=$left simulator-app=$sims"
exit $rc
