#!/usr/bin/env python3
"""Exercise tray SNI and recording UI on a disposable Mango session."""
import argparse
import io
import json
import math
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys
import tempfile
import time

from PIL import Image, ImageDraw


def sni(log):
    if os.environ.get('DBUS_SESSION_BUS_ADDRESS') != f'unix:path={log.parent}/test-bus':
        raise RuntimeError('the SNI fixture requires its private D-Bus address')
    import dbus
    import dbus.service
    from dbus.mainloop.glib import DBusGMainLoop
    from gi.repository import GLib

    DBusGMainLoop(set_as_default=True)
    bus = dbus.SessionBus()
    iface = 'org.kde.StatusNotifierItem'

    class Item(dbus.service.Object):
        def __init__(self, name, title, status, image):
            self.name = dbus.service.BusName(name, dbus.SessionBus(private=True))
            super().__init__(self.name, '/StatusNotifierItem')
            argb = bytes(v for r,g,b,a in image.getdata() for v in (a,r,g,b))
            self.props = dict(Id=title, Title=title, Status=status, IconName='',
                              Menu=dbus.ObjectPath('/'),
                              IconPixmap=dbus.Array([(32, 32, dbus.ByteArray(argb))], signature='(iiay)'))

        @dbus.service.method('org.freedesktop.DBus.Properties', in_signature='ss', out_signature='v')
        def Get(self, interface, name):
            return self.props[name]

        @dbus.service.method('org.freedesktop.DBus.Properties', in_signature='s', out_signature='a{sv}')
        def GetAll(self, interface):
            return self.props

        @dbus.service.method(iface, in_signature='ii')
        def Activate(self, x, y):
            with log.open('a') as stream:
                print(self.props['Id'], file=stream)

    colour = Image.new('RGBA', (32,32))
    painter = ImageDraw.Draw(colour)
    painter.rounded_rectangle((3, 3, 28, 28), 8, fill='#64cca0')
    painter.line([(10, 16),(15, 21),(23, 12)], fill='#173f35', width=3)
    symbol = Image.new('RGBA', (32,32))
    painter = ImageDraw.Draw(symbol)
    painter.rounded_rectangle((5, 7, 26, 23), 4, outline='#eeeeee', width=3)
    painter.line([(11, 27),(21, 27)], fill='#eeeeee', width=3)
    fixtures = [Item('org.dynamite.FixtureSync', 'Sync', 'Active', colour),
                Item('org.dynamite.FixtureDisplay', 'Display', 'NeedsAttention', symbol)]
    watcher = bus.get_object('org.kde.StatusNotifierWatcher', '/StatusNotifierWatcher')
    register = watcher.get_dbus_method('RegisterStatusNotifierItem', 'org.kde.StatusNotifierWatcher')
    for item in fixtures:
        register(item.name.get_name(), timeout=3)
    GLib.MainLoop().run()


def verify(click_tool):
    binary = Path(__file__).resolve().parents[1] / 'target/release/dynamite-rust'
    wallpaper = Path(os.environ.get('DYNAMITE_TEST_WALLPAPER', '/nonexistent-test-wallpaper'))
    with tempfile.TemporaryDirectory(prefix='dynamite-tray-recording-') as tmp:
        root = Path(tmp)
        (root / 'dynamite-rust').mkdir()
        (root / 'dynamite-rust/config.conf').write_text('hover_open=0\ngap_top=13\nbar_height=25\ncollapsed_width=67\n')
        (root / 'mango.conf').write_text('animations = 0\nroot_color = 0x30343aff\n')
        env = {key: value for key, value in os.environ.items() if key not in (
            'DISPLAY','WAYLAND_DISPLAY','WAYLAND_SOCKET','MANGO_INSTANCE_SIGNATURE','SWAYSOCK',
            'DBUS_STARTER_ADDRESS','DBUS_STARTER_BUS_TYPE')}
        address = f'unix:path={root}/test-bus'
        env.update(HOME=tmp, XDG_RUNTIME_DIR=tmp, XDG_CONFIG_HOME=tmp, WLR_BACKENDS='headless',
                   WLR_HEADLESS_OUTPUTS='1', WLR_RENDERER='pixman', GIO_USE_VFS='local',
                   DBUS_SESSION_BUS_ADDRESS=address, PYTHONPATH=os.pathsep.join(sys.path))
        tasks = []

        def start(name, *args):
            with (root / name).open('wb') as log:
                tasks.append(subprocess.Popen(args, env=env, stdout=log, stderr=log, start_new_session=True))

        def run(*args):
            return subprocess.run(args, env=env, check=True, capture_output=True, timeout=5).stdout

        def wait(check):
            end = time.monotonic() + 8
            while time.monotonic() < end:
                result = check()
                if result:
                    return result
                time.sleep(.05)
            raise AssertionError('fixture timed out')

        def shot():
            with Image.open(io.BytesIO(run('grim','-'))) as image:
                return image.convert('RGB')

        def send(message):
            with socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM) as sock:
                sock.sendto(message.encode(), str(root / 'dynamite-rust.sock'))

        try:
            start('dbus.log', 'dbus-daemon', '--session', '--nofork', '--nopidfile', f'--address={address}')
            wait((root / 'test-bus').is_socket)
            start('mango.log', 'mango', '-c', root / 'mango.conf')
            env['WAYLAND_DISPLAY'] = str(wait(lambda: next((p for p in root.glob('wayland-*') if p.is_socket()), None)))
            env['MANGO_INSTANCE_SIGNATURE'] = str(wait(lambda: next(root.glob('mango-*.sock'), None)))
            if wallpaper.is_file():
                start('wallpaper.log','swaybg','-i',wallpaper,'-m','fill')
            time.sleep(.3)
            background = shot()
            start('panel.log', binary)
            wait(lambda: any(x['name']=='dynamite-rust' for x in json.loads(run('mmsg','get','all-layers'))['layers']))
            wait(lambda: 'tray host' in (root/'panel.log').read_text())
            baseline = shot()
            start('sni.log', sys.executable, __file__, '--sni', root/'activations')
            wait(lambda: (root/'panel.log').read_text().count('tray register org.dynamite.Fixture') >= 2)
            time.sleep(1.4)
            collapsed = shot()
            w,h = collapsed.size
            cx = w // 2
            cy = 26
            by = (cx-149, 2, cx+149, 68)
            # The app silhouette is neutral, with no background or badge around it.
            icon = collapsed.crop((cx+40, cy-8, cx+58, cy+8))
            assert any(abs(r-g) <= 1 and abs(g-b) <= 1 and r >= 180 for r,g,b in icon.getdata()), 'neutral icon not painted'
            assert collapsed.getpixel((cx+57, cy)) == baseline.getpixel((cx+57, cy)), 'tray backing still painted'
            collapsed.crop(by).save('/tmp/dynamite-tray-final.png')
            if click_tool:
                run(click_tool, str(cx+45), str(cy), str(w), str(h))
                wait(lambda: (root/'activations').exists() and (root/'activations').read_text() == 'Sync\n')
            else:
                print('SKIP: SNI click needs --click-tool (virtual-pointer executable accepting x y width height)')
            send('dictation_start')
            time.sleep(.3)
            silent = shot()
            n = 0
            end = time.monotonic() + 2.2
            while time.monotonic() < end:
                levels = [max(.01,min(.98, abs(math.sin(i*.32+n*.21))*math.exp(-((i-13)/11)**2))) for i in range(27)]
                send('dictation_bars:'+' '.join(f'{v:.3f}' for v in levels))
                time.sleep(.032)
                n += 1
            spoken = shot()
            bx = cx - 37
            assert spoken.getpixel((cx, 35)) == (0,0,0), 'recording capsule absent'
            meter = (bx+19, 18, bx+55, 32)
            assert silent.crop(meter).tobytes() != spoken.crop(meter).tobytes(), 'waveform did not move'
            # The removed label, badge and timer must leave the surrounding wallpaper alone.
            spoken.crop(by).save('/tmp/dynamite-recording-final.png')
            for point in [(cx-45, 25), (cx, 42)]:
                assert spoken.getpixel(point) == background.getpixel(point), f'recording decoration at {point}: {spoken.getpixel(point)} vs {background.getpixel(point)}'
            send('dictation_end')
            time.sleep(.35)
            assert shot().crop((cx+39,cy-8,cx+61,cy+8)).tobytes() == collapsed.crop((cx+39,cy-8,cx+61,cy+8)).tobytes(), 'tray did not return'
            send('dictation_bars:' + ' '.join(['0.4']*27))
            time.sleep(.3)
            assert shot().getpixel((cx,35)) == (0,0,0), 'bar-only startup did not recover'
            time.sleep(2.7)
            assert shot().crop((cx+39,cy-8,cx+61,cy+8)).tobytes() == collapsed.crop((cx+39,cy-8,cx+61,cy+8)).tobytes(), 'missing end did not recover'
            print('PASS: unboxed neutral tray, compact live waveform, end and dropped-end recovery')
            if click_tool:
                print('PASS: real SNI activation through Wayland pointer delivery')
        except BaseException:
            for path in sorted(root.glob('*.log')):
                print(path.name, path.read_text(errors='replace')[-3200:])
            raise
        finally:
            for proc in reversed(tasks):
                if proc.poll() is None:
                    os.killpg(proc.pid, signal.SIGTERM)
                    try:
                        proc.wait(timeout=4)
                    except subprocess.TimeoutExpired:
                        os.killpg(proc.pid,signal.SIGKILL)
                        proc.wait(timeout=4)


if len(sys.argv)>1 and sys.argv[1]=='--sni':
    sni(Path(sys.argv[2]))
else:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--click-tool', help='virtual-pointer executable accepting x y width height')
    verify(parser.parse_args().click_tool)
