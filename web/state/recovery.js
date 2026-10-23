export function recoverHistory(history,journal) {
    if(!journal) return history;
    if(!journal.assistant || !Array.isArray(journal.calls)) throw new Error('Invalid recovery journal.');
    const messages=[...history,journal.assistant];
    for(const call of journal.calls) messages.push({role:'tool',tool_call_id:call.id,content:call.status==='finished'
        ? String(call.result?.output ?? 'ERROR: missing saved result')
        : call.status==='started' ? 'INTERRUPTED: execution outcome unknown. Inspect workspace files; do not automatically repeat this operation.'
        : 'INTERRUPTED: not executed. No previous approval is valid.'});
    return messages;
}
