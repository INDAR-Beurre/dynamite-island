#!/usr/bin/env python3
"""Notification and OSD regression on a private D-Bus session and headless Mango."""
import io
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]


def mock_daemon():
    from gi.repository import Gio, GLib
    xml = '''<node>
      <interface name="org.freedesktop.Notifications">
        <method name="GetCapabilities"><arg direction="out" type="as"/></method>
        <method name="GetServerInformation"><arg direction="out" type="s"/><arg direction="out" type="s"/><arg direction="out" type="s"/><arg direction="out" type="s"/></method>
        <method name="Notify"><arg direction="in" type="s"/><arg direction="in" type="u"/><arg direction="in" type="s"/><arg direction="in" type="s"/><arg direction="in" type="s"/><arg direction="in" type="as"/><arg direction="in" type="a{sv}"/><arg direction="in" type="i"/><arg direction="out" type="u"/></method>
        <method name="CloseNotification"><arg direction="in" type="u"/></method>
        <signal name="NotificationClosed"><arg type="u"/><arg type="u"/></signal>
      </interface>
      <interface name="org.erikreider.swaync.cc">
        <method name="GetDnd"><arg direction="out" type="b"/></method>
        <method name="SetDnd"><arg direction="in" type="b"/></method>
        <method name="Expire"><arg direction="in" type="u"/></method>
      </interface>
    </node>'''
    bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    for name in ('org.freedesktop.Notifications', 'org.erikreider.swaync.cc'):
        bus.call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus',
                      'RequestName', GLib.Variant('(su)', (name, 0)), GLib.VariantType('(u)'), 0, 5000, None)
    next_id, dnd = 1, False

    def method(connection, sender, path, interface, member, parameters, invocation):
        nonlocal next_id, dnd
        if member == 'Notify':
            values = parameters.unpack()
            notification_id = values[1] or next_id
            next_id = max(next_id, notification_id + 1)
            invocation.return_value(GLib.Variant('(u)', (notification_id,)))
        elif member in ('CloseNotification', 'Expire'):
            notification_id = parameters.unpack()[0]
            bus.emit_signal(None, '/org/freedesktop/Notifications', 'org.freedesktop.Notifications',
                            'NotificationClosed', GLib.Variant('(uu)', (notification_id, 1 if member == 'Expire' else 3)))
            invocation.return_value(GLib.Variant('()', ()))
        elif member == 'GetDnd':
            invocation.return_value(GLib.Variant('(b)', (dnd,)))
        elif member == 'SetDnd':
            dnd = parameters.unpack()[0]
            invocation.return_value(GLib.Variant('()', ()))
        elif member == 'GetCapabilities':
            invocation.return_value(GLib.Variant('(as)', (['body', 'body-markup'],)))
        elif member == 'GetServerInformation':
            invocation.return_value(GLib.Variant('(ssss)', ('Private test daemon', 'Dynamite', '1', '1.2')))

    info = Gio.DBusNodeInfo.new_for_xml(xml)
    bus.register_object('/org/freedesktop/Notifications', info.interfaces[0], method, None, None)
    bus.register_object('/org/erikreider/swaync/cc', info.interfaces[1], method, None, None)
    print('ready', flush=True)
    GLib.MainLoop().run()


def wait_for(predicate, timeout=8):
    until = time.monotonic() + timeout
    while time.monotonic() < until:
        value = predicate()
        if value:
            return value
        time.sleep(.05)
    raise AssertionError('notification self-test timed out')


def main():
    from PIL import Image

    binary = os.environ.get('DYNAMITE_TEST_BINARY', str(ROOT / 'target/release/dynamite-rust'))
    artifacts = Path(os.environ.get('DYNAMITE_TEST_ARTIFACTS', '/tmp/dynamite-notification-proof'))
    artifacts.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='dynamite-notifications-') as temp:
        directory = Path(temp)
        env = {k: v for k, v in os.environ.items() if k not in (
            'DISPLAY', 'WAYLAND_DISPLAY', 'WAYLAND_SOCKET', 'MANGO_INSTANCE_SIGNATURE', 'SWAYSOCK',
            'LD_LIBRARY_PATH', 'LD_PRELOAD')}
        env.update(XDG_RUNTIME_DIR=temp, XDG_CONFIG_HOME=temp, HOME=temp,
                   WLR_BACKENDS='headless', WLR_HEADLESS_OUTPUTS='1', WLR_RENDERER='pixman')
        (directory / 'mango.conf').write_text('animations = 0\n')
        panel_config = directory / 'dynamite-rust'
        panel_config.mkdir()
        (panel_config / 'config.conf').write_text('hover_open=0\ncollapsed_width=67\nbar_height=25\ngap_top=13\nanim_ms=160\n')
        library = Path.home() / '.config/dynamite-rust/library'
        if library.is_dir():
            (panel_config / 'library').symlink_to(library, target_is_directory=True)
        processes = []

        def start(args, name):
            with (directory / name).open('wb') as log:
                child = subprocess.Popen(args, env=env, stdout=log, stderr=log, start_new_session=True)
            processes.append(child)
            return child

        def run(*args):
            return subprocess.run(args, env=env, check=True, capture_output=True, text=True, timeout=8)

        def capture(name):
            shot = subprocess.run(['grim', '-'], env=env, check=True, capture_output=True, timeout=8)
            image = Image.open(io.BytesIO(shot.stdout)).convert('RGB')
            image.save(artifacts / (name + '.png'))
            return image

        def assert_height(name, expected):
            image = capture(name)
            x = image.width // 2
            dark = [y for y in range(10, 510) if max(image.getpixel((x, y))) < 70]
            assert dark, 'panel not visible'
            measured = max(dark) - 13 + 1
            assert abs(measured - expected) <= 2, (name, measured, expected)
            if expected == 25:
                outside = image.crop((image.width // 2 - 280, 46, image.width // 2 + 280, 510))
                assert set(outside.tobytes()) == {144}, 'stale panel pixels after close'
            return image

        def command(state):
            run(binary, state)
            time.sleep(.7)

        def notify(summary, body='Message details', hints=(), replace=None):
            args = ['notify-send', '-p', '-a', 'Mail', '-t', '6000']
            for hint in hints:
                args += ['-h', hint]
            if replace:
                args += ['-r', str(replace)]
            args += [summary, body]
            return int(run(*args).stdout.strip())

        def daemon_method(method, *args):
            return run('gdbus', 'call', '--session', '--dest', 'org.erikreider.swaync.cc',
                       '--object-path', '/org/erikreider/swaync/cc', '--method', 'org.erikreider.swaync.cc.' + method, *map(str, args))

        try:
            bus = subprocess.Popen(['dbus-daemon', '--session', '--nofork', '--print-address=1'],
                                   env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, start_new_session=True)
            processes.append(bus)
            env['DBUS_SESSION_BUS_ADDRESS'] = bus.stdout.readline().strip()
            assert env['DBUS_SESSION_BUS_ADDRESS'].startswith('unix:')
            start(['/usr/bin/python3', str(Path(__file__).resolve()), '--mock-daemon'], 'daemon.log')
            wait_for(lambda: 'ready' in (directory / 'daemon.log').read_text())
            start(['mango', '-c', str(directory / 'mango.conf')], 'mango.log')
            env['WAYLAND_DISPLAY'] = str(wait_for(lambda: next((p for p in directory.glob('wayland-*') if p.is_socket()), None)))
            env['MANGO_INSTANCE_SIGNATURE'] = str(wait_for(lambda: next(directory.glob('mango-*.sock'), None)))
            start(['swaybg', '-c', '#909090'], 'background.log')
            start(['wtype', '-s', '60000'], 'keyboard.log')
            time.sleep(.2)
            panel = start([binary], 'panel.log')
            wait_for(lambda: 'dynamite-rust' in run('mmsg', 'get', 'all-layers').stdout)
            time.sleep(1)
            log = directory / 'panel.log'
            assert_height('idle', 25)
            first = notify('<b>Design review</b>', 'The updated layout is ready.\nPlease take a look.')
            wait_for(lambda: 'Collapsed -> Preview' in log.read_text())
            time.sleep(.7)
            assert_height('notification', 148)
            notify('Updated review', 'Replacement keeps the same notification.', replace=first)
            command('notifications')
            assert_height('one-notification', 188)
            daemon_method('Expire', first)
            time.sleep(.3)
            assert_height('expired-still-reviewable', 188)
            second = notify('Build finished', 'All checks passed.')
            time.sleep(.7)
            assert_height('two-notifications', 278)
            run('gdbus', 'call', '--session', '--dest', 'org.freedesktop.Notifications',
                '--object-path', '/org/freedesktop/Notifications', '--method', 'org.freedesktop.Notifications.CloseNotification', str(second))
            time.sleep(.7)
            assert_height('dismissed', 188)
            run('wtype', '-k', 'Escape')
            time.sleep(.7)
            assert_height('closed-inbox', 25)
            daemon_method('SetDnd', 'true')
            before = log.read_text().count('Collapsed -> Preview')
            notify('Quiet update', 'Visible in the inbox without opening a banner.')
            time.sleep(.8)
            assert log.read_text().count('Collapsed -> Preview') == before, 'DND opened a banner'
            assert_height('dnd', 25)
            command('notifications')
            assert_height('dnd-in-history', 278)
            command('collapsed')
            daemon_method('SetDnd', 'false')
            notify('Sound disabled for this message', hints=('boolean:suppress-sound:true',))
            time.sleep(.7)
            assert_height('silent-audio-banner', 148)
            for value in (15, 35, 75):
                notify(f'Lautstärke: {value}%', hints=('string:x-canonical-private-synchronous:volume',))
                time.sleep(.1)
            time.sleep(.7)
            assert_height('volume', 68)
            notify('Stummgeschaltet', hints=('string:x-canonical-private-synchronous:volume',))
            time.sleep(.65)
            assert_height('muted', 68)
            notify('Helligkeit: 45%', hints=('string:x-canonical-private-synchronous:brightness',))
            time.sleep(.65)
            assert_height('brightness', 68)
            time.sleep(1.9)
            assert_height('osd-closed', 25)
            command('notifications')
            assert_height('history-without-osd', 368)
            for index in range(4):
                notify(f'Inbox message {index}', 'Longer body text wraps within the notification card and stays readable.')
            time.sleep(.7)
            assert_height('inbox-page', 458)
            run('wtype', '-k', 'Down')
            time.sleep(.2)
            capture('inbox-scrolled')
            command('controlcenter')
            baseline = log.read_text().count(' -> Preview')
            notify('Keep menu open', 'An incoming notification must not replace an active menu.')
            time.sleep(.8)
            assert log.read_text().count(' -> Preview') == baseline
            assert_height('controlcenter', 424)
            command('collapsed')
            assert_height('final-idle', 25)
            assert panel.poll() is None
            assert run('mmsg', 'get', 'all-layers').stdout.count('dynamite-rust') == 1
            assert 'notification monitor:' not in log.read_text(), log.read_text()[-3000:]
            (artifacts / 'panel.log').write_text(log.read_text())
            print('PASS: notification capture/replacement/expiry/dismissal, DND, volume/mute/brightness, compact previews, inbox scroll, menu priority and clean close')
        except BaseException:
            for name in ('mango.log', 'panel.log', 'daemon.log'):
                path = directory / name
                if path.exists():
                    print(name, path.read_text(errors='replace')[-4000:])
            raise
        finally:
            for child in reversed(processes):
                if child.poll() is None:
                    os.killpg(child.pid, signal.SIGTERM)
                    try:
                        child.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        os.killpg(child.pid, signal.SIGKILL)
                        child.wait(timeout=5)


if __name__ == '__main__':
    mock_daemon() if '--mock-daemon' in sys.argv else main()
