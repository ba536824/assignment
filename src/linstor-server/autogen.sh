#!/bin/sh
set -e

echo "Generating configuration files for $(basename $PWD)..."

# Run libtoolize if available
if command -v libtoolize >/dev/null 2>&1; then
    libtoolize --copy --force
else
    echo "libtoolize not found, skipping"
fi

# Run aclocal
if command -v aclocal >/dev/null 2>&1; then
    aclocal
else
    echo "aclocal not found, skipping"
fi

# Run autoheader
if command -v autoheader >/dev/null 2>&1; then
    autoheader
else
    echo "autoheader not found, skipping"
fi

# Run automake
if command -v automake >/dev/null 2>&1; then
    automake --add-missing --copy
else
    echo "automake not found, skipping"
fi

# Run autoconf
if command -v autoconf >/dev/null 2>&1; then
    autoconf
else
    echo "autoconf not found, skipping"
fi

echo "Configuration files generated successfully."
