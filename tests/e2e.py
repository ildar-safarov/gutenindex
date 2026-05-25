import json
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).parent.parent
INDEXER_CLI = REPO / "target/release/indexer-cli"
SEARCH_CLI = REPO / "target/release/search-cli"
CORPUS_DIR = REPO / "corpus"


def run(*args, **kwargs):
    result = subprocess.run(args, capture_output=True, text=True, **kwargs)
    if result.returncode != 0:
        print(result.stderr, file=sys.stderr)
        raise RuntimeError(f"Command failed: {args}")
    return result.stdout


def phrase_found(phrase, doc_id, index_path):
    words = phrase.lower().split()
    locs_per_word = []
    for word in words:
        data = json.loads(run(str(SEARCH_CLI), "--index", str(index_path), "--word", word))
        doc_locs = next((set(p["locations"]) for p in data if p["doc_id"] == doc_id), None)
        if doc_locs is None:
            return False
        locs_per_word.append(doc_locs)
    for start in locs_per_word[0]:
        pos = start
        for i, word in enumerate(words):
            if pos not in locs_per_word[i]:
                break
            pos += len(word) + 1
        else:
            return True
    return False


def main():
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)

        print("Step 1: cargo build --release")
        run("cargo", "build", "--release", cwd=REPO)

        print("Step 2: collect-ascii --limit 1500")
        manifest = tmp / "manifest.json"
        run(
            str(INDEXER_CLI), "tools", "collect-ascii",
            "--corpus-dir", str(CORPUS_DIR / "zips"),
            "--limit", "1500",
            "--save-indexer-input-json-to", str(manifest),
        )
        doc_ids = set(json.loads(manifest.read_text())["doc_ids"])
        assert 0 in doc_ids, "doc_id 0 not found"
        assert 1000 in doc_ids, "doc_id 1000 not found"
        print(f"  collected {len(doc_ids)} docs, 0 and 1000 present")

        print("Step 3: build index")
        index = tmp / "index"
        run(
            str(INDEXER_CLI), "index",
            "--indexer-input", str(manifest),
            "--corpus-dir", str(CORPUS_DIR / "zips"),
            "--output-db", str(index),
        )

        print("Step 4: phrase search doc 0")
        phrase0 = "What joy they take to fill their hands with that delightful wool"
        assert phrase_found(phrase0, 0, index), f"Phrase not found in doc 0: {phrase0!r}"
        print("  found")

        print("Step 5: phrase search doc 1000")
        phrase1000 = "There were two other clerks besides Ben"
        assert phrase_found(phrase1000, 1000, index), f"Phrase not found in doc 1000: {phrase1000!r}"
        print("  found")

    print("PASS")


if __name__ == "__main__":
    main()
