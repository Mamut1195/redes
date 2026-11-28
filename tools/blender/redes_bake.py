"""Shared driver for family recipes: one Blender process bakes one or many parameter sets.

A recipe `X.py` lists its parameters in `X.receta.json` (the contract the CLI also reads,
docs/17 section F17-02) and calls `run(__file__, validate, build)`. The params file is either
one parameter set or `{"jobs": [set, ...]}`; every set carries its own `output` path and, if it
also has `blend`, the scene is saved there for review in Blender (with the parameters on it).

There are no default values on purpose: a missing parameter stops the bake instead of being
filled in (D16), so the CLI reports it and the viewer keeps the fallback piece.
"""

import json
import os
import sys

import bpy

OUTPUT_KEYS = {"output", "blend"}


def recipe_param_names(script_path):
    recipe = os.path.splitext(script_path)[0] + ".receta.json"
    with open(recipe, encoding="utf-8") as f:
        return set(json.load(f)["parametros"])


def read_jobs(argv):
    if "--" not in argv:
        sys.exit("expected: -- <params.json>")
    # utf-8-sig: PowerShell 5.1 writes JSON with a BOM.
    with open(argv[argv.index("--") + 1], encoding="utf-8-sig") as f:
        given = json.load(f)
    if isinstance(given, dict) and "jobs" in given:
        return given["jobs"]
    return [given]


def check_names(job, names):
    missing = sorted(names - set(job))
    if "output" not in job:
        missing.append("output")
    if missing:
        sys.exit(f"missing params: {missing}")
    unknown = sorted(set(job) - names - OUTPUT_KEYS)
    if unknown:
        sys.exit(f"unknown params: {unknown}")


def export_glb(path):
    bpy.ops.export_scene.gltf(
        filepath=path,
        export_format="GLB",
        export_apply=True,
        export_extras=True,
        export_yup=True,
        export_materials="EXPORT",
    )


def run(script_path, validate, build):
    names = recipe_param_names(script_path)
    jobs = read_jobs(sys.argv)
    # Every set is checked before the first bake, so a bad set never leaves half a batch behind.
    for job in jobs:
        check_names(job, names)
        validate(job)
    for job in jobs:
        # Each set starts from an empty scene: datablock names stay the same as in a
        # one-set process, which keeps batch output byte-identical to single bakes.
        bpy.ops.wm.read_factory_settings(use_empty=True)
        build(job)
        export_glb(job["output"])
        if "blend" in job:
            save_review_blend(job, script_path)


def save_review_blend(job, script_path):
    # Only after the export: scene custom properties would otherwise reach the .glb as extras.
    params = {k: v for k, v in sorted(job.items()) if k not in OUTPUT_KEYS}
    scene = bpy.context.scene
    scene["redes_receta"] = os.path.basename(script_path)
    scene["redes_parametros"] = json.dumps(params, sort_keys=True)
    for name, value in params.items():
        scene[f"redes_{name}"] = value
    note = bpy.data.texts.new("redes_receta.txt")
    note.write(review_note(script_path, params))
    # The .blend embeds timestamps: it is for review only, never part of the cache key.
    bpy.ops.wm.save_as_mainfile(filepath=job["blend"], compress=False)


def review_note(script_path, params):
    return (
        f"Baked by tools/blender/{os.path.basename(script_path)} (docs/17, section F17-02).\n"
        "The cover is a Geometry Nodes modifier: its inputs can be edited in the modifier panel.\n"
        "The body is explicit geometry: to change it, edit the parameters and bake again with\n"
        "  blender -b --factory-startup --python-exit-code 1 --python "
        f"tools/blender/{os.path.basename(script_path)} -- params.json\n\n"
        f"params.json:\n{json.dumps(params, indent=2, sort_keys=True)}\n"
    )
