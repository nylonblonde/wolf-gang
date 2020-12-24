# Workflow for importing models from Blender

1. Animate body, name and push action to NLA
2. Apply constraints to other parts (ie: tails require a child-of the root bone, and heads require copy transform of the head bone),
3. Bake animations (in pose mode, Pose > Animation > Bake Animation, set to visual keying, clear constraints, and apply to pose)
4. Name and push baked actions to NLA (with the naming convention of BaseAnimation.BodyPart)
5. Mute NLAs
6. Export using Godot Engine (.escn) exporter, export stashed animations, animation as actions