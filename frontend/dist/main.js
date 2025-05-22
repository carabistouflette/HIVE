const promptInputEl = document.querySelector("#prompt-input");
const agentSelectEl = document.querySelector("#agent-select");
const llmModelSelectEl = document.querySelector("#llm-model-select");
const executeTaskButtonEl = document.querySelector("#execute-task-button");
const taskStatusEl = document.querySelector("#task-status");
const taskResultEl = document.querySelector("#task-result");

if (executeTaskButtonEl) {
    executeTaskButtonEl.addEventListener("click", async () => {
        const { invoke } = window.__TAURI__.core;

        const prompt = promptInputEl.value;
        const agent = agentSelectEl.value;
        const llmModel = llmModelSelectEl.value;

        if (!prompt) {
            alert("Veuillez saisir un prompt pour la tâche.");
            return;
        }

        taskStatusEl.textContent = "En cours...";
        taskResultEl.textContent = "Exécution de la tâche...";
        executeTaskButtonEl.disabled = true;

        try {
            const result = await invoke("execute_agent_task", {
                prompt: prompt,
                agent: agent,
                llmModel: llmModel,
            });
            console.log("Task execution result:", result);
            taskStatusEl.textContent = "Terminée";
            taskResultEl.textContent = result;
        } catch (error) {
            console.error("Error executing task:", error);
            taskStatusEl.textContent = "Échec";
            taskResultEl.textContent = `Erreur : ${error}`;
        } finally {
            executeTaskButtonEl.disabled = false;
        }
    });
}