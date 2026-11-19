// A deterministic external teacher. The test supplies each decision and the
// context it expects in a newly started ACP process.
const fs = require("node:fs");
const readline = require("node:readline");
const turn = JSON.parse(fs.readFileSync(process.argv[2], "utf8"));
const reply = value => process.stdout.write(JSON.stringify({jsonrpc:"2.0", ...value}) + "\n");
const update = data => reply({method:"session/update", params:{sessionId:"classroom", update:data}});

readline.createInterface({input:process.stdin}).on("line", line => {
    const {id, method, params} = JSON.parse(line);
    if (method === "initialize") {
        reply({id, result:{protocolVersion:1, agentCapabilities:{}}});
    } else if (method === "session/new") {
        if (!params.mcpServers.some(server => server.name === "understand-book")) process.exit(10);
        reply({id, result:{sessionId:"classroom"}});
    } else if (method === "session/prompt") {
        const prompt = params.prompt[0].text;
        if (!prompt.includes("one JSON object") || !prompt.includes(turn.context)) process.exit(11);
        update({sessionUpdate:"tool_call",toolCallId:"private-check",title:"PRIVATE_TEACHER_REFERENCE",status:"completed"});
        const raw = typeof turn.decision === "string" ? turn.decision : JSON.stringify(turn.decision);
        update({sessionUpdate:"agent_message_chunk", content:{type:"text", text:raw}});
        reply({id, result:{stopReason:"end_turn"}});
    }
});
