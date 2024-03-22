#!/bin/bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: The Ferrocene Developers

set -euo pipefail
IFS=$'\n\t'

# All of our Python scripts have `#!/usr/bin/env python3` as their shebang,
# but python3.exe does not exist on Windows by default. Thus we copy python.exe
# to python3.exe if the latter is not present.
setup_python3() {
    if ! command -v python3.exe >/dev/null; then
        if ! command -v python.exe >/dev/null; then
            echo "error: could not find python.exe on the system"
            exit 1
        fi

        python="$(command -v python.exe)"
        cp "${python}" "$(dirname "${python}")/python3.exe"
    fi
}

if [[ "${OSTYPE}" != "msys" ]]; then
    echo "error: the script can only run on Windows (under MSYS)"
    exit 1
fi

setup_python3

choco install -y cmake ninja zstandard

# By default CMake is not in the bash PATH, so we need to add
# it manually to it.
echo 'export PATH="/c/Program Files/CMake/bin:${PATH}"' >> "${BASH_ENV}"

where git
where tar
ls /c/Program\ Files\
exit 1