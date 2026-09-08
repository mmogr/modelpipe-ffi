#!/usr/bin/env bash
#
# Validate .github/workflows/*.yml for duplicate mapping keys.
#
# Taken from gglib's script of the same name, minus its second half (which
# checks badge module paths this repo has no equivalent of).
#
# GitHub rejects a workflow file containing a duplicate key outright: the run is
# marked "failed because of a workflow file issue" and NO jobs start. That makes
# it invisible to CI itself — a broken ci.yml cannot run the job that would have
# caught it — so this check has to happen before the push.
#
# Most YAML parsers won't help: the spec says duplicate keys are invalid, but
# Psych's safe_load and PyYAML both silently keep the last one. Walking the raw
# node tree is what makes them visible.
set -euo pipefail

cd "$(dirname "$0")/.."

if ! command -v ruby >/dev/null 2>&1; then
  # Locally a missing ruby is a shrug; in CI it would turn this check into a
  # silent pass forever — and this is the one check that cannot save you
  # after the push. Fail loudly there instead.
  if [ -n "${CI:-}" ]; then
    echo "✗ ruby not found and CI is set — refusing to silently skip workflow YAML validation" >&2
    exit 1
  fi
  echo "⚠ ruby not found — skipping workflow YAML validation"
  exit 0
fi

ruby -ryaml -e '# encoding: utf-8
#
# Load-bearing, and the first line for a reason: ruby takes the encoding of
# a -e script from the locale, so under LC_ALL=C or a bare POSIX default it
# is US-ASCII and the two glyphs below are a SyntaxError before a single
# workflow file is read. Setting -E utf-8 or RUBYOPT does not fix it: those
# set the external and internal encodings, not the encoding of the script
# itself. Measured — without this line,
# `LC_ALL=C ./scripts/check_workflow_yaml.sh` dies with
# "invalid multibyte char (US-ASCII)" and exits 1, so `make pre-commit`
# fails on any machine whose locale is not already UTF-8. GitHub runners
# set LANG=C.UTF-8, which is why this hid from CI.
#
# Note this whole script is a single-quoted shell string, so it must contain
# no apostrophe anywhere.
bad = 0
Dir.glob(".github/workflows/*.yml").sort.each do |file|
  begin
    doc = YAML.parse(File.read(file))
  rescue Psych::SyntaxError => e
    puts "  \e[31m✗\e[0m #{file}: #{e.message}"
    bad += 1
    next
  end
  next unless doc

  walk = lambda do |node, path|
    if node.is_a?(Psych::Nodes::Mapping)
      seen = {}
      node.children.each_slice(2) do |k, v|
        key = (k.respond_to?(:value) ? k.value : k.to_s)
        if seen[key]
          puts "  \e[31m✗\e[0m #{file}:#{k.start_line + 1} duplicate key \x27#{key}\x27 in #{path.empty? ? "(root)" : path} (first seen at line #{seen[key]})"
          bad += 1
        end
        seen[key] = k.start_line + 1
        walk.call(v, "#{path}/#{key}")
      end
    elsif node.respond_to?(:children) && node.children
      node.children.each { |c| walk.call(c, path) }
    end
  end
  walk.call(doc, "")
end

count = Dir.glob(".github/workflows/*.yml").length
if bad.zero?
  puts "\e[32m✓\e[0m no duplicate keys in #{count} workflow file(s)"
else
  puts "\e[31m#{bad} problem(s) found — GitHub would reject these and run no jobs at all\e[0m"
  exit 1
end
'

# Every `run:` block must be valid shell.
#
# The failure this exists for: a `run:` block is a string as far as YAML and as
# far as GitHub are concerned, so a shell syntax error inside one is invisible
# to every check above and to the whole of CI. It surfaces only when the step
# runs — and on a release workflow that is after the build, the smoke test and
# the checksum have all succeeded, seventeen minutes in, on the one run that
# was supposed to publish something.
#
# That is not hypothetical. `git -c user.name="Matt O'"'"'Grady"` used the
# single-quote-escaping idiom while already inside double quotes, left the
# quoting unbalanced, and killed the first real release at its second-to-last
# step. `bash -n` on the rendered block finds it in milliseconds.
#
# Rendered, not raw: YAML strips the block scalar's indentation, so the string
# the shell actually receives is not the text in the file. Checking the file
# would miss heredoc terminators that only reach column 0 after that strip.
python3 - <<'PY'
import glob
import os
import subprocess
import sys
import tempfile

try:
    import yaml
except ImportError:
    # Same reasoning as the ruby check above: locally a shrug, in CI a lie.
    if os.environ.get("CI"):
        print("\033[31m✗\033[0m PyYAML not found and CI is set — refusing to skip", file=sys.stderr)
        sys.exit(1)
    print("\033[33m⚠\033[0m PyYAML not found — skipping run-block shell validation")
    sys.exit(0)

bad = 0
checked = 0
for path in sorted(glob.glob(".github/workflows/*.yml")):
    with open(path) as handle:
        document = yaml.safe_load(handle)
    for job_name, job in (document.get("jobs") or {}).items():
        for step in job.get("steps") or []:
            script = step.get("run")
            if not script:
                continue
            checked += 1
            with tempfile.NamedTemporaryFile("w", suffix=".sh", delete=False) as scratch:
                scratch.write(script)
                scratch_path = scratch.name
            result = subprocess.run(
                ["bash", "-n", scratch_path], capture_output=True, text=True
            )
            os.unlink(scratch_path)
            if result.returncode != 0:
                bad += 1
                detail = result.stderr.strip().splitlines()
                print(
                    f"\033[31m✗\033[0m {path} → {job_name} → {step.get('name', '(unnamed)')}",
                    file=sys.stderr,
                )
                for line in detail[-2:]:
                    print(f"    {line.split(': ', 1)[-1]}", file=sys.stderr)

if bad:
    print(
        f"\033[31m{bad} run block(s) are not valid shell — the step would die when it ran\033[0m",
        file=sys.stderr,
    )
    sys.exit(1)
print(f"\033[32m✓\033[0m all {checked} run block(s) are valid shell")
PY
