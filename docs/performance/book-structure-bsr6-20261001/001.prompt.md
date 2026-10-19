# BookStructure provisional whole-book outline

Read the canonical chapter titles, accepted anchored overviews and delivered preface.
Describe the question each chapter answers and how the book develops. Preserve every unit in source order,
including cover/preface units. Copy unit_lid as an identity; never calculate chapter numbers from it.
This is a revisable outline, not a claim of body coverage or a reading prerequisite graph.
Return JSON only:
{"chapters":[{"unit_lid":"...","question":{"text":"...","evidence_lids":["..."]},"progression":{"text":"...","evidence_lids":["..."]}}],"themes":[{"question":{"text":"...","evidence_lids":["..."]},"unit_lids":["..."]}]}
Each chapter explanation cites its own delivered overview or preface evidence. Each tentative theme cites every member.
Use the book's language. Keep each question and progression concise (normally one sentence); themes describe distinct questions.
Do not output stops, dependencies or rewritten candidate bodies. Do not infer evidence from title numbers.
