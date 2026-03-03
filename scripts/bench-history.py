# /// script
# requires-python = ">=3.13"
# dependencies = [
#     "requests>=2.32.5",
# ]
# ///
import json

REPO = "skim-rs/skim"

from requests import get
import os
import tempfile
import shutil
import subprocess
import tarfile
import zipfile
import stat


def _extract_release(rel):
    return {
        "date": rel["created_at"],
        "version": rel["tag_name"],
        "download_url": next(
            x["browser_download_url"]
            for x in rel["assets"]
            if ("linux" in x["name"] and "x86" in x["name"])
        ),
    }


def get_releases():
    releases = []
    page_offset = 1
    while True:
        page = get(
            f"https://api.github.com/repos/{REPO}/releases?per_page=100&page={page_offset}",
            headers={
                "Accept": "application/vnd.github+json",
                "X-GitHub-Api-Version": "2022-11-28",
                "Authorization": f"Bearer {os.environ['GH_API_KEY']}",
            },
        ).json()
        if len(page) > 0:
            releases += page
            page_offset += 1
        else:
            break
    return list(map(_extract_release, releases))


def download(rel):
    # Download the release binary and unpack it in a temporary file
    url = rel["download_url"]
    dst_dir = tempfile.mkdtemp(prefix="skim-release-")
    try:
        resp = get(url, stream=True)
        resp.raise_for_status()

        # Try to determine filename
        fname = None
        cd = resp.headers.get("content-disposition")
        if cd and "filename=" in cd:
            fname = cd.split("filename=")[-1].strip('"')
        if not fname:
            fname = os.path.basename(url.split("?")[0]) or "asset"

        file_path = os.path.join(dst_dir, fname)
        with open(file_path, "wb") as f:
            for chunk in resp.iter_content(chunk_size=8192):
                if chunk:
                    f.write(chunk)

        # If archive, extract
        lower = file_path.lower()
        extracted_root = dst_dir
        if lower.endswith(".zip"):
            with zipfile.ZipFile(file_path, "r") as z:
                z.extractall(dst_dir)
        elif lower.endswith((".tar.gz", ".tgz", ".tar.xz", ".tar")):
            with tarfile.open(file_path, "r:*") as t:
                t.extractall(dst_dir)
        else:
            # Assume it's a raw binary; mark executable and return
            st = os.stat(file_path)
            os.chmod(file_path, st.st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
            return file_path, dst_dir

        # Find an executable in extracted tree. Prefer name containing 'sk' or 'skim'
        candidates = []
        for root, _, files in os.walk(extracted_root):
            for fn in files:
                p = os.path.join(root, fn)
                try:
                    if os.path.isfile(p) and os.access(p, os.X_OK):
                        candidates.append(p)
                except Exception:
                    continue

        # Try to pick best candidate
        def score(pth: str) -> int:
            n = os.path.basename(pth).lower()
            if "sk" == n or n == "sk" or n.startswith("sk"):
                return 100
            if "skim" in n:
                return 90
            return 10

        if not candidates:
            # If nothing marked executable, try to make some files executable and pick largest
            files_all = [
                os.path.join(root, f)
                for root, _, fs in os.walk(extracted_root)
                for f in fs
            ]
            if not files_all:
                raise RuntimeError(
                    f"no files found in archive for release {rel.get('version')}"
                )
            # pick largest file
            files_all = sorted(
                files_all, key=lambda p: os.path.getsize(p), reverse=True
            )
            p = files_all[0]
            st = os.stat(p)
            os.chmod(p, st.st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
            return p, dst_dir

        candidates.sort(key=score, reverse=True)
        chosen = candidates[0]
        # ensure executable bit set
        st = os.stat(chosen)
        os.chmod(chosen, st.st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
        return chosen, dst_dir
    except Exception:
        # cleanup on error
        try:
            shutil.rmtree(dst_dir)
        except Exception:
            pass
        raise


def bench(path):
    # Run bench.sh on the binary at `path` with 3 runs in json mode and return the json output as a dict
    repo_root = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    bench_sh = os.path.join(repo_root, "bench.sh")
    fixture_path = os.path.join(repo_root, "benches", "fixtures", "10M.txt")
    if not os.path.isfile(bench_sh):
        raise RuntimeError("bench.sh not found in repo root")

    cmd = [bench_sh, path, "-r", "10", "-j", "-f", fixture_path]
    # Ensure bench.sh is executable
    st = os.stat(bench_sh)
    os.chmod(bench_sh, st.st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)

    proc = subprocess.run(cmd, cwd=repo_root, capture_output=True, text=True)
    # bench.sh prints the JSON to stdout when -j passed
    out = proc.stdout.strip()
    if proc.returncode != 0:
        # include stderr for debugging
        raise RuntimeError(
            f"bench.sh failed: {proc.returncode}\nstdout:\n{out}\nstderr:\n{proc.stderr}"
        )

    try:
        return json.loads(out)
    except Exception as e:
        raise RuntimeError(f"failed to parse bench output as json: {e}\noutput:\n{out}")


def run_benches(releases):
    # Download and bench each release and save the results to a json file
    results = []
    for i, rel in enumerate(releases, start=1):
        print(f"[{i}/{len(releases)}] processing {rel.get('version')}")
        try:
            bin_path, tmpdir = download(rel)
        except Exception as e:
            print(f"failed to download {rel.get('version')}: {e}")
            results.append(
                {
                    "version": rel.get("version"),
                    "date": rel.get("date"),
                    "download_url": rel.get("download_url"),
                    "error": f"download: {e}",
                }
            )
            # persist intermediate results
            with open(out_path, "w") as f:
                json.dump(results, f, indent=2)
            continue

        try:
            bench_out = bench(bin_path)
        except Exception as e:
            print(f"bench failed for {rel.get('version')}: {e}")
            bench_out = {"error": str(e)}
        finally:
            # cleanup downloaded/extracted files
            try:
                shutil.rmtree(tmpdir)
            except Exception:
                pass

        record = {
            "version": rel.get("version"),
            "date": rel.get("date"),
            "download_url": rel.get("download_url"),
            "bench": bench_out,
        }
        results.append(record)

        out_path = "/tmp/bench-history.json"
        with open(out_path, "w") as f:
            json.dump(results, f, indent=2)
    return results


def main() -> None:
    releases = [r for r in get_releases() if r["date"].startswith("2026")]
    # with open("/tmp/releases.json", "r") as f:
    #     releases = json.load(f)

    print(f"fetched {len(releases)} releases")

    out_path = "/tmp/bench-history.json"

    results = run_benches(releases)
    with open(out_path, "w") as f:
        json.dump(results, f, indent=2)

    # with open(out_path, "r") as f:
    #     results = json.load(f)


if __name__ == "__main__":
    main()
