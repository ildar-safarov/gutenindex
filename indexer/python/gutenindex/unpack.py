import argparse
import json
import logging
import multiprocessing as mp
import os
import tempfile
import zipfile
from functools import partial
from pathlib import Path
from typing import Dict, List

from tqdm import tqdm


def process_one_file(docs_pg_root: Path, zip_to_doc_id: Dict[Path, int], zip_path: Path) -> None:
    doc_id = zip_to_doc_id[zip_path]

    dst_path = docs_pg_root / "doc_files" / str(doc_id % 10) / f"{doc_id}.txt"
    if dst_path.exists():
        return

    txt_name = f"{zip_path.stem}.txt"

    with zipfile.ZipFile(zip_path, "r") as zip_ref:
        info_we_need = None
        for info in zip_ref.filelist:
            if os.path.basename(info.filename) == txt_name:
                info_we_need = info
                break

        if info_we_need is None:
            logging.warning(f"File named {txt_name} wasn't found in {zip_path}")
            return

        with tempfile.TemporaryDirectory() as tmp_dir:
            try:
                zip_ref.extract(info_we_need, tmp_dir)
            except Exception as e:
                logging.warning(f"I've got error while processing {zip_path}: {e}", exc_info=True)
                return

            os.makedirs(dst_path.parent, exist_ok=True)
            os.rename(Path(tmp_dir) / info_we_need.filename, dst_path)


def main():
    parser = argparse.ArgumentParser(description="Unpack Project Gutenberg ZIP files into text files")
    parser.add_argument("pg_raw_root", type=Path, help="Directory containing raw Gutenberg ZIP files")
    parser.add_argument("docs_pg_root", type=Path, help="Output directory for extracted text files")
    args = parser.parse_args()

    pg_raw_root: Path = args.pg_raw_root
    docs_pg_root: Path = args.docs_pg_root

    doc_id_to_orig_path_json = docs_pg_root / "doc_id_to_orig_path.json"

    if not doc_id_to_orig_path_json.exists():
        zips: List[Path] = []
        for root, _, files in os.walk(pg_raw_root):
            for f in files:
                if f.endswith(".zip"):
                    zips.append(Path(root) / f)
        zips = sorted(zips)

        zip_to_doc_id: Dict[Path, int] = dict()
        for i, z in enumerate(zips):
            zip_to_doc_id[z] = i

        orig_paths: Dict[int, str] = dict()
        for z in zips:
            orig_paths[zip_to_doc_id[z]] = "/".join(z.parts[len(pg_raw_root.parts):])

        os.makedirs(doc_id_to_orig_path_json.parent, exist_ok=True)
        with open(doc_id_to_orig_path_json, "w") as f:
            json.dump(orig_paths, f, indent=2)
    else:
        with open(doc_id_to_orig_path_json) as f:
            orig_paths = json.load(f)

        zips = []
        zip_to_doc_id = dict()
        for doc_id, relative_path in orig_paths.items():
            zip_path = pg_raw_root / relative_path
            zips.append(zip_path)
            zip_to_doc_id[zip_path] = doc_id

    with mp.Pool() as pool:
        for _ in tqdm(pool.imap(partial(process_one_file, docs_pg_root, zip_to_doc_id), zips), total=len(zips)):
            pass


if __name__ == '__main__':
    main()
