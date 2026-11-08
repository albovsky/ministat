"""Exercise the administration-Mac boot-service flow without sudo, SSH or launchd.

Fake `gh` and `ssh` commands stand in for GitHub and the monitored host. The fake
SSH strips `/usr/bin/sudo`, sets SUDO_USER/SUDO_UID as sudo would and redirects
only the root staging parent into a test directory, then runs the real wrapper.
"""
import os
import pathlib
import shlex
import shutil
import subprocess
import tempfile
import unittest

HERE = pathlib.Path(__file__).parent
SHA = 'a' * 40

FAKE_GH = r'''#!/bin/sh
case "$*" in
  *compare/*) echo "$FAKE_COMPARE" ;;
  *contents/scripts/install-boot-service.sh*) cat "$FAKE_HELPER" ;;
  *) echo main ;;
esac
'''

FAKE_SSH = r'''#!/usr/bin/env python3
import os, pathlib, shlex, subprocess, sys
args = sys.argv[1:]
assert args[:2] == ['-t', '--'], args
pathlib.Path(os.environ['FAKE_SSH_LOG']).write_text(args[2])
command = shlex.split(args[3])
assert command[:3] == ['/usr/bin/sudo', '/bin/sh', '-c'], command[:3]
wrapper = command[3]
assert wrapper.count('/private/var/root/ministat.XXXXXXXX') == 1
wrapper = wrapper.replace('/private/var/root/ministat.XXXXXXXX',
                          shlex.quote(os.environ['FAKE_ROOT'] + '/ministat.XXXXXXXX'))
rest = command[4:]
if os.environ.get('FAKE_CORRUPT'):
    rest[1] = rest[1][:-4] + 'AAAA'
env = {'HOME': os.environ['HOME'], 'PATH': '/usr/bin:/bin',
       'SUDO_USER': 'monitored', 'SUDO_UID': '501'}
sys.exit(subprocess.run(['/bin/sh', '-c', wrapper, *rest], env=env).returncode)
'''


class AdminBootServiceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = pathlib.Path(self.temporary.name)
        self.home = self.root / "home with ' quote"
        self.home.mkdir()
        scripts = self.root / 'project/scripts'
        scripts.mkdir(parents=True)
        for name in ('admin-boot-service.sh', 'boot-service-command.sh'):
            shutil.copyfile(HERE / name, scripts / name)
        self.scripts = scripts
        # The local checkout's helper is hostile; it must never run.
        (scripts / 'install-boot-service.sh').write_text(
            '#!/bin/sh\ntouch "$HOME/local-helper-ran"\n')
        self.helper = self.root / 'github-helper.sh'
        self.helper.write_text(
            '#!/bin/sh\nset -eu\nprintf "%s\\n" "$0" "$1" "$2" "$3" > "$HOME/executed"\n')
        bin_dir = self.root / 'bin'
        bin_dir.mkdir()
        for name, body in (('gh', FAKE_GH), ('ssh', FAKE_SSH)):
            (bin_dir / name).write_text(body)
            (bin_dir / name).chmod(0o755)
        self.ssh_log = self.root / 'ssh-host'
        self.env = {**os.environ, 'HOME': str(self.home), 'TMPDIR': str(self.root),
                    'MINISTAT_GH': str(bin_dir / 'gh'), 'MINISTAT_SSH': str(bin_dir / 'ssh'),
                    'FAKE_HELPER': str(self.helper), 'FAKE_COMPARE': 'behind',
                    'FAKE_ROOT': str(self.root), 'FAKE_SSH_LOG': str(self.ssh_log)}

    def admin(self, *args, **env):
        return subprocess.run(['/bin/sh', str(self.scripts / 'admin-boot-service.sh'), *args],
                              env={**self.env, **env}, capture_output=True, text=True, timeout=20)

    def executed(self):
        return (self.home / 'executed').read_text().splitlines()

    def test_github_helper_runs_as_root_copy_for_the_sudo_account(self):
        result = self.admin('macmini', SHA)
        self.assertEqual(result.returncode, 0, result.stderr)
        script, user, uid, mode = self.executed()
        self.assertEqual((user, uid, mode), ('monitored', '501', 'install'))
        self.assertEqual(pathlib.Path(script).parent.parent, self.root)
        self.assertFalse(pathlib.Path(script).parent.exists())
        self.assertFalse((self.home / 'local-helper-ran').exists())
        self.assertEqual(self.ssh_log.read_text(), 'macmini')
        self.assertIn(f'/blob/{SHA}/scripts/install-boot-service.sh', result.stderr)

    def test_remove_mode_is_forwarded(self):
        result = self.admin('--remove', 'macmini', SHA)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.executed()[3], 'remove')

    def test_unmerged_commit_is_refused_unless_explicitly_allowed(self):
        for status in ('ahead', 'diverged'):
            result = self.admin('macmini', SHA, FAKE_COMPARE=status)
            self.assertEqual(result.returncode, 1)
            self.assertIn('is not on main', result.stderr)
            self.assertFalse(self.ssh_log.exists())
        result = self.admin('--unmerged', 'macmini', SHA, FAKE_COMPARE='ahead')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('without checking that it is merged', result.stderr)

    def test_ambiguous_refs_and_option_like_hosts_are_rejected(self):
        for args in (('macmini', 'main'), ('macmini', SHA[:12]), ('macmini', SHA.upper()),
                     ('-oProxyCommand=x', SHA)):
            result = self.admin(*args)
            self.assertEqual(result.returncode, 2, args)
        self.assertFalse(self.ssh_log.exists())

    def test_payload_damaged_in_transit_never_executes(self):
        result = self.admin('macmini', SHA, FAKE_CORRUPT='1')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('did not arrive intact', result.stderr)
        self.assertFalse((self.home / 'executed').exists())

    def test_host_side_command_only_prints_instructions(self):
        output = subprocess.check_output(
            ['/bin/sh', str(self.scripts / 'boot-service-command.sh'), '--remove'],
            env=self.env, text=True)
        self.assertIn('admin-boot-service.sh --remove', output)
        self.assertNotIn('sudo', output)
        self.assertFalse((self.home / 'local-helper-ran').exists())


if __name__ == '__main__':
    unittest.main()
