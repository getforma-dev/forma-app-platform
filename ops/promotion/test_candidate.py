import copy
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
from ops.promotion import candidate as c

SHA = "a" * 40


def runs():
    return [{"id": i + 1, "path": ".github/workflows/" + name, "event": "push", "head_sha": SHA,
        "repository": {"id": c.REPOSITORY_ID}, "head_branch": c.BRANCH, "run_number": 1,
        "run_attempt": 1, "status": "completed", "conclusion": "success"} for i, name in enumerate(sorted(c.REQUIRED))]


class CandidateTests(unittest.TestCase):
    def test_new_quality_workflow_cannot_be_silently_omitted(self):
        source = Path(__file__).resolve().parents[2] / ".github/workflows"
        self.assertEqual({p.name for p in source.glob("*.yml")} - {"beta-promote.yml"}, c.REQUIRED)

    def test_immediate_gate_refuses_trunk_movement_during_ci_read(self):
        replies = [{"object": {"sha": SHA}}, {"workflow_runs": runs()}, {"object": {"sha": "b" * 40}}]
        with patch.object(c, "api", side_effect=replies), self.assertRaisesRegex(ValueError, "superseded"):
            c.check_current(SHA)

    def test_exact_required_push_set_excludes_promotion_without_deadlock(self):
        rows = runs()
        rows.append(dict(rows[0], path=c.WORKFLOW, status="in_progress", conclusion=None))
        ready, selected = c.ci_status(rows, SHA)
        self.assertTrue(ready)
        self.assertEqual(len(selected), 12)

    def test_pr_and_other_sha_do_not_satisfy_required_push(self):
        for field, value in (("head_sha", "b" * 40), ("event", "pull_request")):
            rows = runs()
            rows[0][field] = value
            self.assertFalse(c.ci_status(rows, SHA)[0])

    def test_new_failed_or_cancelled_attempt_cannot_reuse_old_success(self):
        for conclusion in ("failure", "cancelled", "skipped", "timed_out"):
            rows = runs()
            rows.append(dict(rows[0], run_attempt=2, conclusion=conclusion))
            with self.assertRaisesRegex(ValueError, "CI failed"):
                c.ci_status(rows, SHA)

    def test_foreign_repo_or_branch_is_refused(self):
        for field, value in (("repository", {"id": 12}), ("head_branch", "pivot/pitch-mvp")):
            rows = runs()
            rows[0][field] = value
            with self.assertRaises(ValueError):
                c.ci_status(rows, SHA)

    @patch.dict('os.environ', {'GITHUB_EVENT_NAME': 'push'})
    def test_event_requires_exact_identity_and_ordinary_trunk_push(self):
        event = {"repository": {"full_name": c.REPOSITORY, "id": c.REPOSITORY_ID,
            "owner": {"id": c.OWNER_ID}}, "ref": "refs/heads/" + c.BRANCH, "after": SHA,
            "deleted": False, "forced": False, "before": "b" * 40}
        self.assertEqual(c.context(event, SHA), SHA)
        with patch.dict('os.environ', {'GITHUB_EVENT_NAME': 'pull_request'}):
            with self.assertRaisesRegex(ValueError, 'push event required'):
                c.context(event, SHA)
        for field, value in (("ref", "refs/pull/1/merge"), ("forced", True), ("deleted", True), ("after", "b" * 40), ("before", "0" * 40)):
            altered = dict(event, **{field: value})
            with self.assertRaises(ValueError):
                c.context(altered, SHA)

    @unittest.skipUnless(shutil.which('git'), 'actual Git history regression requires git')
    def test_rebase_merge_with_final_docs_commit_and_runtime_rename(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            def git(*args):
                return subprocess.check_output(['git', '-C', str(root), *args], text=True, stderr=subprocess.DEVNULL).strip()
            def commit(message):
                git('add', '.')
                git('-c', 'user.name=Promotion Test', '-c', 'user.email=promotion@example.invalid', 'commit', '-m', message)
                return git('rev-parse', 'HEAD')
            git('init'); (root/'README.md').write_text('initial\n'); before = commit('base')
            git('-c', 'user.name=Promotion Test', '-c', 'user.email=promotion@example.invalid',
                'tag', '-a', 'beta/20261101-' + before[:7], '-m', 'synthetic verified baseline')
            (root/'app/src').mkdir(parents=True); (root/'app/src/feature.rs').write_text('pub fn feature() {}\n')
            commit('application change')
            (root/'README.md').write_text('document application change\n'); after = commit('docs last')
            with patch.object(c, 'git', side_effect=git):
                self.assertTrue(any(c.runtime_path(p) for p in c.changed_paths(before, after)))
                self.assertFalse(any(c.runtime_path(p) for p in c.changed_paths(git('rev-parse', 'HEAD^'), after)))
                published, pending = c.unpublished_paths(after)
                self.assertEqual(published, before)
                self.assertTrue(any(c.runtime_path(p) for p in pending), 'docs must not hide an unpublished app merge')
                (root/'app/src/feature.rs').rename(root/'example.md'); renamed = commit('move runtime file to docs')
                self.assertIn('app/src/feature.rs', c.changed_paths(after, renamed))
                with self.assertRaisesRegex(ValueError, 'not an ancestor'):
                    c.changed_paths(renamed, before)

    def test_missing_published_reference_never_proves_docs_only(self):
        with patch.object(c, 'git', return_value=''):
            self.assertEqual(c.unpublished_paths(SHA), (None, None))

    def test_doc_only_merge_does_not_redeploy_but_ui_assets_do(self):
        for path in ("README.md", "docs/pitch/RELEASES.md", "Tickets/APP-167.md", "ops/promotion/README.md", "evidence/chart.png"):
            self.assertFalse(c.runtime_path(path), path)
        for path in ("app/admin/src/canvas/unite.ts", "app/admin/src/styles/canvas.css", "app/admin/logo.svg",
                "infra/src/app.ts", c.WORKFLOW, "ops/promotion/github.py"):
            self.assertTrue(c.runtime_path(path), path)

    def test_candidate_is_merged_commit_not_an_open_pr_head(self):
        pr = {"number": 55, "merged_at": "2026-11-02T00:00:00Z", "merge_commit_sha": SHA,
            "base": {"ref": c.BRANCH, "repo": {"id": c.REPOSITORY_ID}}}
        self.assertEqual(c.merged_pr([pr], SHA), 55)
        for altered in (dict(pr, merged_at=None), dict(pr, merge_commit_sha="b" * 40), dict(pr, base={"ref": "pivot/pitch-mvp"})):
            with self.assertRaises(ValueError):
                c.merged_pr([altered], SHA)


if __name__ == "__main__":
    unittest.main()
