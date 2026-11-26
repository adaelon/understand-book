// A deterministic external teacher. The test supplies each decision and the
// context it expects in a newly started ACP process.
const fs = require("node:fs");
const readline = require("node:readline");
const turn = JSON.parse(fs.readFileSync(process.argv[2], "utf8"));
let hasReaderTools = false;
const reply = value => process.stdout.write(JSON.stringify({jsonrpc:"2.0", ...value}) + "\n");
const update = data => reply({method:"session/update", params:{sessionId:"classroom", update:data}});

readline.createInterface({input:process.stdin}).on("line", line => {
    const {id, method, params} = JSON.parse(line);
    if (method === "initialize") {
        reply({id, result:{protocolVersion:1, agentCapabilities:{}}});
    } else if (method === "session/new") {
        hasReaderTools = params.mcpServers.some(server => server.name === "understand-book");
        reply({id, result:{sessionId:"classroom"}});
    } else if (method === "session/prompt") {
        const prompt = params.prompt[0].text;
        if (turn.teach_back) {
            if (hasReaderTools && turn.help && prompt.endsWith(turn.question)) {
                update({sessionUpdate:"agent_message_chunk", content:{type:"text", text:turn.help}});
                reply({id, result:{stopReason:"end_turn", usage:{totalTokens:20}}});
                return;
            }
            const packet = JSON.parse(prompt.slice(prompt.lastIndexOf("\n\n") + 2));
            if (hasReaderTools || fs.readdirSync(process.cwd()).length) process.exit(15);
            fs.appendFileSync(turn.record, JSON.stringify({cwd: process.cwd(), packet}) + "\n");
            const draft = packet.expression ? turn.student : packet.input ? turn.corrected : turn.draft;
            update({sessionUpdate:"agent_message_chunk", content:{type:"text", text:JSON.stringify(draft)}});
            reply({id, result:{stopReason:"end_turn", usage:{totalTokens:15}}});
            return;
        }
        if (prompt.startsWith("Check one proposed transfer exercise")) {
            if (!turn.comparison || hasReaderTools || prompt.includes(turn.decision.reason) || prompt.includes(turn.learner)) process.exit(14);
            fs.appendFileSync(turn.record, "comparison\n");
            update({sessionUpdate:"agent_message_chunk", content:{type:"text", text:JSON.stringify(turn.comparison)}});
            reply({id, result:{stopReason:"end_turn", usage:{totalTokens:15}}});
            return;
        }
        if (prompt.startsWith("Independently assess only the submitted learner_response")) {
            if (!turn.review || hasReaderTools || prompt.includes(turn.decision.reason) || !prompt.includes(turn.learner)) process.exit(13);
            fs.appendFileSync(turn.record, "review\n");
            update({sessionUpdate:"agent_message_chunk", content:{type:"text", text:JSON.stringify(turn.review)}});
            reply({id, result:{stopReason:"end_turn", usage:{totalTokens:15}}});
            return;
        }
        if (!hasReaderTools) process.exit(10);
        if (!prompt.includes("one JSON object") || !prompt.includes(turn.context)) process.exit(11);
        const repairing = prompt.includes("Reader validation:");
        if (turn.record) {
            fs.appendFileSync(turn.record, repairing ? "revision\n" : "initial\n");
            if (repairing && ![turn.validation, turn.rejected].every(text => prompt.includes(text))) process.exit(12);
        }
        update({sessionUpdate:"tool_call",toolCallId:"private-check",title:"PRIVATE_TEACHER_REFERENCE",status:"completed"});
        const decision = repairing ? (turn.revision ?? turn.decision) : turn.decision;
        const raw = typeof decision === "string" ? decision : JSON.stringify(decision);
        update({sessionUpdate:"agent_message_chunk", content:{type:"text", text:raw}});
        reply({id, result:{stopReason:turn.stop ?? "end_turn", usage:{totalTokens:15}}});
    }
});
