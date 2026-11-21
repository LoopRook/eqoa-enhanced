import json, tempfile, unittest
from pathlib import Path
import eqoa1_freeze as f


def surface(rows):
    return {"capabilities": [{"path": p, "status": s, "children": c} for p, s, c in rows]}


class Freeze(unittest.TestCase):
    def run_update(self, rows, record=None, allow=False):
        with tempfile.TemporaryDirectory() as d:
            d = Path(d)
            (d / "s.json").write_text(json.dumps(surface(rows)))
            if record is not None:
                (d / "r.json").write_text(json.dumps({"schema_version": 1, "api": "eqoa/1", "supported": record}))
            code = f.update(d / "s.json", d / "r.json", allow)
            out = json.loads((d / "r.json").read_text()) if (d / "r.json").exists() else None
            return code, out

    def test_creates_and_only_adds(self):
        code, out = self.run_update([("a", "supported", ["a.b"]), ("a.b", "supported", []), ("x", "unsupported", [])])
        self.assertEqual(code, 0)
        self.assertEqual(out["supported"], {"a": "group", "a.b": "leaf"})
        code, out = self.run_update([("a", "supported", []), ("n", "supported", [])], {"a": "leaf"})
        self.assertEqual((code, out["supported"]), (0, {"a": "leaf", "n": "leaf"}))

    def test_refuses_removal_downgrade_and_kind_change(self):
        for rows in ([], [("a", "unsupported", [])], [("a", "supported", ["a.b"])]):
            code, out = self.run_update(rows, {"a": "leaf"})
            self.assertEqual(code, 1)
            self.assertEqual(out["supported"], {"a": "leaf"})  # untouched

    def test_override_applies_the_downgrade(self):
        code, out = self.run_update([("a", "unsupported", []), ("n", "supported", [])], {"a": "leaf"}, True)
        self.assertEqual((code, out["supported"]), (0, {"n": "leaf"}))


if __name__ == "__main__":
    unittest.main()
