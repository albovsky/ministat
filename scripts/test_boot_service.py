"""Exercise the printed verifier without sudo or changes to launchd.

Only the root staging parent is redirected into a test directory. The real
copy/checksum/execute sequence runs as the test user against a harmless helper.
"""
import os
import pathlib
import shlex
import shutil
import subprocess
import tempfile
import unittest


class BootServiceCommandTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = pathlib.Path(self.temporary.name)
        self.home = self.root / "home with ' quote"
        self.home.mkdir()
        self.scripts = self.root / "project/scripts"
        self.scripts.mkdir(parents=True)
        shutil.copyfile(pathlib.Path(__file__).with_name('boot-service-command.sh'),
                        self.scripts / 'boot-service-command.sh')
        (self.scripts / 'install-boot-service.sh').write_text(
            '#!/bin/sh\nset -eu\nprintf "%s\\n" "$0" > "$HOME/executed"\n')
        self.environment = {**os.environ, 'HOME': str(self.home)}
        output = subprocess.check_output(
            ['/bin/sh', str(self.scripts / 'boot-service-command.sh')],
            env=self.environment, text=True)
        self.command = shlex.split(output)
        self.assertEqual(self.command[:3], ['/usr/bin/sudo', '/bin/sh', '-c'])
        self.candidate = pathlib.Path(self.command[5])

    def run_verifier(self):
        wrapper = self.command[3]
        self.assertEqual(wrapper.count('/private/var/root/ministat.XXXXXXXX'), 1)
        wrapper = wrapper.replace('/private/var/root/ministat.XXXXXXXX',
                                  shlex.quote(str(self.root / 'ministat.XXXXXXXX')))
        return subprocess.run(
            ['/bin/sh', '-c', wrapper, *self.command[4:]],
            env=self.environment, capture_output=True, text=True, timeout=10)

    def test_verified_copy_executes_and_is_removed_afterwards(self):
        result = self.run_verifier()
        self.assertEqual(result.returncode, 0, result.stderr)
        executed = pathlib.Path((self.home / 'executed').read_text().strip())
        self.assertNotEqual(executed, self.candidate)
        self.assertEqual(executed.parent.parent, self.root)
        self.assertFalse(executed.parent.exists())

    def test_changed_candidate_never_executes(self):
        self.candidate.write_text('#!/bin/sh\ntouch "$HOME/executed"\n')
        result = self.run_verifier()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('installer changed', result.stderr)
        self.assertFalse((self.home / 'executed').exists())

    def test_replaced_candidate_symlink_never_executes(self):
        replacement = self.root / 'replacement.sh'
        replacement.write_text('#!/bin/sh\ntouch "$HOME/executed"\n')
        self.candidate.unlink()
        self.candidate.symlink_to(replacement)
        result = self.run_verifier()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('installer changed', result.stderr)
        self.assertFalse((self.home / 'executed').exists())


if __name__ == '__main__':
    unittest.main()
