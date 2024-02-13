# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: The Ferrocene Developers

from . import outcomes, render_template, render_summary

def setup(app):
    outcomes.setup(app)
    render_template.setup(app)
    render_summary.setup(app)

    return {
        "version": "0",
        "parallel_read_safe": True,
        "parallel_write_safe": True,
    }
