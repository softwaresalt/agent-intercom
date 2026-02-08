# **Feasibility Study: Remote Orchestration of GitHub Copilot Agent Sessions via Interactive Chat Channels**

## **1\. Executive Summary**

The convergence of generative AI and integrated development environments (IDEs) has culminated in the "Agentic" workflow, where developers no longer merely type code but orchestrate AI agents to plan, execute, and verify complex engineering tasks. GitHub Copilot, operating within Visual Studio Code (VS Code), represents the pinnacle of this capability. However, a significant operational gap exists: these agents currently require the developer's physical presence at the console to initiate prompts, provide context, and grant security approvals for file manipulations or terminal executions. This constraint tethers the developer to the workstation, negating the asynchronous potential of AI-driven development.  
This report evaluates the feasibility of decoupling the developer from the local execution environment by establishing a secure, bi-directional command-and-control bridge between the local VS Code instance and remote communication platforms: Microsoft Teams, Slack, or Telegram. The objective is to enable a developer to "manage" Copilot sessions—prompting, approving, and responding—via a mobile-friendly chat interface while the heavy computational work occurs on a secure work laptop.  
Our analysis concludes that a direct "remote control" of the native VS Code Chat UI panel is technically infeasible due to the lack of accessibility APIs exposing the internal state of the Chat View to extensions. However, a **functional equivalent is highly feasible** and recommended. By leveraging the recently stabilized **VS Code Language Model API (vscode.lm)**, a custom extension can be engineered to act as a "Shadow Agent" or "Bridge." This extension operates within the VS Code Extension Host, consumes the GitHub Copilot gpt-4o models programmatically, manages context via workspace APIs, and routes interactions through a persistent WebSocket connection to a chat platform.  
Among the evaluated platforms, **Slack via Socket Mode** is identified as the optimal implementation pathway. It offers the unique combination of firewall-traversing connectivity (requiring no inbound ports or VPNs), a rich interactive user interface (Block Kit) essential for safe "Approve/Reject" workflows, and enterprise-grade security compliance. This report details the architectural blueprint for this "Slack-to-Copilot Bridge," analyzes the necessary VS Code API surfaces, and provides a comprehensive implementation strategy to realize remote agentic orchestration.

## **2\. Architectural Context and Requirement Analysis**

### **2.1 The Shift to Agentic Development**

The release of "Agent Mode" (often invoked via @workspace or specific agent extensions) in VS Code transforms the IDE from a text editor into a task execution environment. In this paradigm, the user provides a high-level intent (e.g., "Refactor the authentication middleware to support JWTs"), and the agent iteratively analyzes the codebase, plans changes, and executes edits. This interaction model is inherently conversational and multi-turn, requiring the user to act as a "Human-in-the-Loop" supervisor rather than a typist.  
The user's requirement to perform this supervision remotely implies a need for a "Headless" mode of operation—a capability that VS Code natively supports via its Extension Host architecture, but which has not yet been exposed via a turnkey "Remote Agent" feature.

### **2.2 Requirement Decomposition**

To satisfy the user's request, the proposed system must satisfy four distinct functional requirements:

1. **Workspace Initialization:** The system must be able to "wake up" or interact with an active workspace on the work laptop.  
2. **Session Orchestration:** The user must be able to initiate a new session or continue a context-aware conversation with the agent.  
3. **Routing:** Interactions (text, code blocks, diffs) must flow securely between the local VS Code process and the remote chat platform (Teams, Slack, or Telegram).  
4. **Governance (Prompt/Approve/Respond):** The system must support interactive elements. Approvals cannot be simple text commands; they require secure, unambiguous signals (e.g., button presses) to authorize file writes or terminal commands.

### **2.3 The Core Feasibility Constraint: UI vs. API**

A critical distinction in this feasibility study is the difference between the **Copilot Chat View (UI)** and the **Copilot Language Model (API)**.  
Research into the VS Code API surface reveals that the native Chat View (the panel where users typically type) is a "black box" to other extensions. There is no API that allows an extension to:

* Read the text currently displayed in the Chat View.  
* Programmatically "type" into the input box of the native Chat View.  
* "Click" the "Apply in Editor" button on behalf of the user.

While commands like workbench.action.chat.open exist to open the panel , they do not return a handle to the session object. Therefore, an approach that attempts to "remote control" the graphical interface via accessibility scripts (like PyAutoGUI) would be brittle, insecure, and functionally limited.  
**The Viable Alternative:** The feasibility of this project rests entirely on the **VS Code Language Model API (vscode.lm)**. Introduced to allow extension authors to build *their own* AI features, this API provides direct access to the models that power GitHub Copilot (e.g., gpt-4o, gpt-3.5-turbo).  
Consequently, the solution is not to bridge the *native UI* to Slack, but to build a **Bridge Extension** that:

1. Receives a message from Slack.  
2. Passes it to the vscode.lm API (using Copilot as the backend).  
3. Receives the text/code response.  
4. Sends it back to Slack.  
5. Handles tool invocations (like writing files) by asking for permission in Slack and then executing them via vscode.workspace APIs.

This distinction changes the scope from "Remote Desktop for Chat" to "Building a Custom Headless Agent," which is a supported and robust development path.

## **3\. Deep Dive: VS Code AI Extensibility Architecture**

To validate the engineering approach, we must examine the specific capabilities and limitations of the VS Code APIs that will underpin the Bridge Extension.

### **3.1 The Language Model API (vscode.lm)**

The vscode.lm namespace is the engine room of this solution. It allows extensions to query Large Language Models (LLMs) available in the editor.  
**Model Selection:** The API provides selectChatModels, which allows the extension to request a specific backend. To utilize the user's existing GitHub Copilot subscription, the extension filters for the vendor copilot.

* **Selector:** { vendor: 'copilot', family: 'gpt-4o' }.  
* **Availability:** This returns a LanguageModelChat object if the user is signed into GitHub Copilot. If the user is offline or not signed in, this returns an empty array, requiring the Bridge Extension to handle connection states gracefully.

**Request Mechanics:** Once a model is selected, the extension uses sendRequest. This method accepts a stream of messages (LanguageModelChatMessage) and returns a streaming response (LanguageModelChatResponse).

* **Streaming:** The response is asynchronous and chunked. This aligns perfectly with the "typing..." indicators in chat apps. The Bridge Extension can stream these chunks to Slack in real-time or buffer them to update the message payload periodically.  
* **Roles:** The API supports User and Assistant roles, allowing the extension to maintain a conversation history buffer. This is crucial for "multi-turn" interactions where the developer asks follow-up questions.

**Security and Consent:** A major feasibility factor is the **User Consent Dialog**. The VS Code security model dictates that extensions cannot access the user's GitHub Copilot models without explicit permission.

* **The Check:** LanguageModelAccessInformation.canSendRequest(model) returns a boolean indicating if permission has been granted.  
* **The Blocking Event:** On the very first request, VS Code summons a modal dialog: *"Extension 'Bridge Agent' wants to use GitHub Copilot. Allow?"*.  
* **Implication:** This creates a "First-Run Requirement." The developer *must* be physically present at the laptop to click "Allow" the first time the extension runs. Once granted, this permission is persisted in the global state. If the extension is updated or the auth token expires, this prompt may reappear, potentially breaking the remote session. The Bridge Extension must implement a "Health Check" status that reports to Slack if it loses permission, prompting the user to return to the laptop.

### **3.2 Context Gathering and Tool Invocation**

A raw LLM has no knowledge of the user's workspace. The "Magic" of Copilot Agent Mode is its ability to search files and understand project structure. To replicate this remotely, the Bridge Extension must provide **Context**.  
**The vscode.lm.tools API:** Recent updates to VS Code (late 2024/early 2025\) introduced the concept of **Language Model Tools**. Extensions can register tools that the LLM can "call".

* **Invoking Existing Tools:** The research indicates that built-in tools like copilot\_searchCodebase are registered in the system. However, programmatically invoking these tools from another extension (the Bridge) via vscode.lm.invokeTool is a complex and evolving surface area.  
* **Tool Discovery:** The Bridge Extension can query vscode.lm.tools to see what capabilities are available. If copilot\_searchCodebase is exposed, the Bridge can pass this tool definition to the model during the sendRequest call.  
* **Agentic Loop:** When the LLM decides it needs to search the codebase, it returns a ToolCall response instead of text. The Bridge Extension must detect this, invoke the tool (or ask the user if the tool is sensitive), and feed the result back to the LLM. This "Tool Calling Loop" is the engine of the agent.

**Alternative Context Strategy:** If the built-in Copilot tools are restricted (private API), the Bridge Extension can implement its own simple RAG (Retrieval-Augmented Generation) or file-reading tools using vscode.workspace.findFiles and vscode.workspace.fs.readFile. This ensures the agent can still "read" code even if it cannot access Copilot's proprietary index.

### **3.3 The Chat Participant API (vscode.chat)**

The research mentions vscode.chat.createChatParticipant. It is important to clarify that this API is for **creating new agents** that live in the VS Code UI. It is *not* for controlling existing ones.

* **Relevance:** The Bridge Extension effectively acts as a Chat Participant, but its "UI" is Slack, not the VS Code sidebar. We will likely use logic similar to a Chat Participant implementation but route the I/O to the WebSocket rather than the ChatResponseStream.

## **4\. The Core Solution: The "Shadow Agent" Bridge Extension**

Based on the API analysis, the recommended technical solution is a custom VS Code Extension designed specifically for remote orchestration. We will refer to this as the **"Shadow Bridge."**

### **4.1 Architecture Overview**

The architecture is a "Hub-and-Spoke" model where the Work Laptop acts as the Hub, and the Remote Chat App acts as the Spoke.

1. **The Host:** The Work Laptop runs VS Code (or code-server / VS Code Tunnels).  
2. **The Extension:** The Shadow Bridge extension activates on startup (onStartupFinished).  
3. **The Transport:** The extension initiates a secure **Outbound WebSocket** connection to the Chat Platform (e.g., Slack). This bypasses the need for inbound firewall rules or public IP addresses.  
4. **The Brain:** The extension holds a reference to the Copilot Model via vscode.lm.  
5. **The State:** The extension maintains an in-memory or persisted conversation history (Context Window) for the active session.

### **4.2 Handling the "Session" Concept**

The user query asks to "manage agent sessions." In VS Code, a "session" is effectively the conversation history.

* **Persistence:** The Shadow Bridge must store the history of the conversation. Since the native Chat View history is inaccessible, the Bridge must create its own history object: Array\<LanguageModelChatMessage\>.  
* **Multi-Session Support:** The extension can map "Slack Threads" to "VS Code Sessions."  
  * *Implementation:* When a user replies in a specific Slack Thread, the extension retrieves the conversation history associated with that Thread ID. This allows the user to have multiple parallel refactoring tasks running simultaneously in different threads.

### **4.3 The Execution Lifecycle (Prompt \-\> Approve \-\> Respond)**

The critical workflow for a remote agent is the **Authorization Loop**.

1. **Prompt:** User types "Refactor login.ts to use async/await" in Slack.  
2. **Processing:** Bridge sends prompt to Copilot Model with read-access to login.ts.  
3. **Proposal:** Copilot generates the refactored code.  
4. **Review (Crucial Step):** The Bridge does *not* apply the code. It generates a **diff** (using vscode.diff logic) and sends a formatted message to Slack showing the "Before" and "After".  
5. **Approval Request:** The Slack message contains "Approve" and "Reject" buttons (Block Kit).  
6. **Action:**  
   * If **Approve**: The Bridge creates a WorkspaceEdit and applies it via vscode.workspace.applyEdit. It then reports "Changes applied."  
   * If **Reject**: The Bridge drops the changes and asks for feedback.

This "Human-in-the-Loop" architecture satisfies the user's requirement to "approve and respond" and mitigates the risk of an AI hallucinating destructive file changes while the user is away.

## **5\. Channel Feasibility and Protocol Analysis**

This section evaluates Teams, Slack, and Telegram against the specific constraints of corporate firewalls, API capabilities, and the "interactive" requirement.

### **5.1 Slack (Recommended)**

Slack is the superior candidate for this implementation due to its **Socket Mode** and **Block Kit** framework.  
**Connectivity: Socket Mode**

* **Mechanism:** The Bridge Extension connects to wss://wss-primary.slack.com/.... This is a standard outbound WebSocket connection over port 443\.  
* **Firewall Traversal:** Corporate firewalls almost universally allow outbound HTTPS/WSS to recognized domains like Slack. No VPN, no ngrok, and no IT exceptions are typically required.  
* **Implementation:** The @slack/bolt library or a lightweight WebSocket client can run directly inside the VS Code Extension Host (Node.js environment).

**Interaction: Block Kit**

* **Capability:** Slack supports rich JSON-defined UI elements. The Bridge can send a "Block" containing a formatted code snippet (using markdown backticks) followed by Actions blocks containing Green "Approve" and Red "Deny" buttons.  
* **State:** When a button is clicked, Slack sends an event back over the WebSocket. The extension can correlate this event with the pending WorkspaceEdit using a callback\_id.

**Verdict:** High feasibility, best UI, easiest traversal.

### **5.2 Telegram**

Telegram is a viable alternative, particularly for personal projects or stricter networks, but lacks the "Enterprise" polish of Slack.  
**Connectivity: Long Polling**

* **Mechanism:** The extension executes a while(true) loop, calling the getUpdates API endpoint via HTTPS.  
* **Firewall Traversal:** Highly robust. It looks like standard web browsing traffic.  
* **Limitations:** Long polling introduces latency (latency between the user typing and the agent receiving).

**Interaction: Inline Keyboards**

* **Capability:** Telegram supports "Inline Keyboards" (buttons under messages).  
* **Constraint:** Code formatting in Telegram is limited to basic Markdown/HTML. Displaying complex diffs (red/green lines) is difficult. Diffs might need to be sent as .diff file attachments rather than inline text, slowing down the review process.

**Verdict:** Medium feasibility. Good connectivity, but poor UI for code review.

### **5.3 Microsoft Teams**

Teams is the least feasible option for a purely "laptop-hosted" implementation due to its connectivity architecture.  
**Connectivity: The Webhook Problem**

* **Mechanism:** The Azure Bot Framework primarily operates via Webhooks. Teams sends an HTTP POST to the bot. This requires the laptop to have a **publicly accessible URL**.  
* **The Tunneling Requirement:** To run this locally, the user must run a tunnel (like dev tunnels or ngrok) to expose the extension's local server to the internet.  
  * *Risk:* Tunnels are frequently blocked by corporate firewalls. Tunnels expire. Tunnels introduce a "Man-in-the-Middle" security surface.  
* **No Native Socket Mode:** Unlike Slack, Teams does not offer a production-ready "Socket Mode" for bots that bypasses the need for an Azure endpoint.

**Verdict:** Low feasibility. Requires significant infrastructure (Azure Bot Service) or fragile tunneling.

## **6\. Remote Orchestration Mechanics**

How does the system handle the complexity of a coding session (Context, Terminal, Approvals) through a chat window?

### **6.1 Managing Context remotely**

In the IDE, the user opens files to give the agent context. Remotely, the user cannot "open" a tab.

* **Explicit Context Strategy:** The user must explicitly reference files in the chat.  
  * *User:* "Read src/utils.ts and explain the error handler."  
  * *Bridge:* Regex matches read \<filename\>. Calls vscode.workspace.findFiles to locate it, reads content, and appends it to the system prompt of the LLM as context.  
* **Implicit Context (Project Map):** The extension can periodically generate a "file tree" summary and keep it in the LLM's context window, so the agent understands the directory structure without needing to read every file.

### **6.2 Managing Approvals and Safety**

To prevent the agent from destroying the codebase while the user is AFK (Away From Keyboard), a **Strict Permission Model** is implemented in the Bridge.

| Action Type | Risk Level | Behavior |
| :---- | :---- | :---- |
| **Chat/Question** | Low | Auto-replies. No approval needed. |
| **File Read** | Low | Auto-executes (if file is within workspace). |
| **File Write/Edit** | High | **Blocking Approval.** Bridge sends Diff to Chat. User must click "Approve". |
| **Terminal Command** | Critical | **Blocking Approval.** Bridge sends command text to Chat. User must click "Run". |

This logic is implemented in the LanguageModelTool handler. When the LLM generates a tool call for editFile, the Bridge intercepts it, suspends the execution, sends the interactive message to Slack, and waits for the WebSocket event block\_actions with the matching ID.

### **6.3 Handling "Workspaces"**

The user requirement mentions "set up a workspace."

* **Scenario:** The laptop is rebooted. VS Code is closed.  
* **Limitation:** A VS Code Extension cannot run if VS Code is not running.  
* **Solution: VS Code Tunnels (Service Mode).** The user should install the "Remote \- Tunnels" service (code tunnel service install) on the laptop. This allows the machine to run a "headless" VS Code Server that starts on boot. The Bridge Extension can be installed in this headless remote profile.  
  * *Result:* The "Agent" is available as soon as the laptop has internet, even if the graphical desktop is locked or no window is open.

## **7\. Implementation Strategy: The Slack Socket Bridge**

This section outlines the concrete steps to implement the recommended solution.

### **7.1 Prerequisites**

1. **Work Laptop:** VS Code installed, GitHub Copilot active subscription.  
2. **Slack Workspace:** Permission to create a generic "App".  
3. **Node.js:** Installed on the laptop (for extension development/runtime).

### **7.2 Phase 1: The Slack App Setup**

1. **Create App:** Go to api.slack.com. Create "Copilot Bridge".  
2. **Socket Mode:** Enable "Socket Mode". Copy the app-level token (xapp-...).  
3. **Permissions:** Add scopes chat:write, im:history, app\_mentions:read.  
4. **Install:** Install to Workspace. Copy the bot token (xoxb-...).

### **7.3 Phase 2: The VS Code Extension**

Generate a new extension using yo code.  
**package.json Configuration:**  
`{`  
  `"name": "copilot-slack-bridge",`  
  `"activationEvents":,`  
  `"engines": {`  
    `"vscode": "^1.90.0" // Required for vscode.lm API`  
  `}`  
`}`

**extension.ts Logic (Simplified Narrative):**

1. **Initialization:** On activate, the extension instantiates a SocketModeClient (from @slack/bolt or ws). It authenticates using the tokens (stored securely in context.secrets).  
2. **Listener:** It subscribes to the message event from Slack.  
3. **Handler:**  
   * It extracts the text from the Slack message.  
   * It calls vscode.lm.selectChatModels({ vendor: 'copilot' }).  
   * It checks model.access.canSendRequest. If false, it sends a "Help" message to Slack: *"Please open VS Code on the laptop and approve the Copilot permission dialog."*  
   * It creates a LanguageModelChatMessage.User with the text.  
   * It calls model.sendRequest.  
4. **Streaming:** It hooks into the response stream. To avoid Slack rate limits (approx. 1 update/sec), it buffers the text chunks and updates the Slack message via chat.update every \~1.5 seconds rather than on every token.  
5. **Tool Handling:** If the response includes a tool call (e.g., suggested\_edit), it constructs a Block Kit JSON payload with the diff and buttons, sending it as a new message.

### **7.4 Phase 3: The "Diff" Renderer**

To make code reviews on mobile feasible, the extension utilizes a "Unified Diff" format.

* *Implementation:* The extension reads the current file, applies the proposed edit in memory, and generates a standard \+ / \- diff string.  
* *Presentation:* This string is wrapped in a markdown code block \`\`\`diff... \`\`\` inside the Slack message. This provides color coding (Green/Red) natively in the Slack mobile app.

### **7.5 Phase 4: Security Hardening**

To prevent unauthorized access:

* **User ID Locking:** The extension configuration must include a allowedSlackUserIds setting. The listener checks message.user against this list. If the ID doesn't match, it silently drops the message.  
* **Command Filtering:** The Bridge should explicitly block dangerous shell commands (rm \-rf, format) if implementing a terminal runner.

## **8\. Risk Assessment and Mitigation**

### **8.1 Data Privacy (Data Loss Prevention)**

* **Risk:** Code snippets from the proprietary workspace are being sent to Slack's servers.  
* **Mitigation:** This architecture is compliant only if the organization uses **Slack Enterprise Grid** (where data retention policies apply) or if the project is non-sensitive. For strict IP protection, this solution might violate DLP policies.  
* **Technical Control:** The extension can be configured to "Redact" sensitive patterns (API keys, PII) before sending chunks to Slack, though this limits utility.

### **8.2 Authentication State Drift**

* **Risk:** The GitHub Copilot token in VS Code expires, or the session locks.  
* **Mitigation:** The Bridge Extension monitors vscode.authentication.onDidChangeSessions. If the session becomes invalid, it proactively sends a "Connection Lost" alert to Slack. It cannot *fix* it remotely (requires browser login), but it provides visibility.

### **8.3 Cost**

* **Slack:** Socket Mode is free for standard workspaces.  
* **Copilot:** Uses the standard subscription limits. High-frequency automated prompting might trigger rate limits on the Copilot side. The Bridge handles 429 Too Many Requests errors by pausing and retrying, notifying the user in Slack.

## **9\. Recommendation and Conclusion**

To achieve remote management of GitHub Copilot sessions, we recommend the **development of a custom "Shadow Bridge" VS Code Extension utilizing Slack Socket Mode.**  
**Rationale:**

1. **Feasibility:** It aligns with the documented capabilities of the vscode.lm API (programmatic model access) while avoiding the limitations of the vscode.chat API (blocked UI access).  
2. **Connectivity:** Slack Socket Mode is the only reliable method to penetrate corporate firewalls without violating security policies regarding inbound ports or tunnels.  
3. **Usability:** The Block Kit UI allows for genuine "Management" (Approve/Reject) rather than just passive reading.

**Implementation Path:** The user should begin by scaffolding a VS Code extension that implements a simple "Echo Bot" via Slack Socket Mode. Once connectivity is established, integrate the vscode.lm API to forward prompts to Copilot. Finally, implement the WorkspaceEdit logic to handle the "Approve" button callbacks. This approach provides a secure, robust, and highly functional remote agent interface.

### **Summary of Best Approach:**

* **Platform:** Slack (Free Tier or Enterprise).  
* **Transport:** Socket Mode (WebSockets).  
* **Local Agent:** Custom VS Code Extension (vscode.lm).  
* **Context:** vscode.workspace file reading \+ Manual Context injection.  
* **Approval:** Slack Block Kit Buttons triggering applyEdit.

This solution transforms the work laptop into a secure AI server, accessible from anywhere, satisfying the user's need for remote, interactive agent orchestration.

## **10\. Technical Appendix: JSON Payloads and API References**

### **10.1 Slack Block Kit "Approval" Payload**

The following JSON structure demonstrates how the Bridge Extension requests approval for a code change.  
`{`  
  `"blocks":`  
    `}`  
  `]`  
`}`

### **10.2 VS Code Language Model Request Implementation**

The TypeScript implementation for the extension's core logic.  
`import * as vscode from 'vscode';`

`async function queryCopilot(userPrompt: string): Promise<string> {`  
    `// 1. Select the Copilot Model (GPT-4o)`  
    `const models = await vscode.lm.selectChatModels({`  
        `vendor: 'copilot',`  
        `family: 'gpt-4o'`  
    `});`

    `if (models.length === 0) {`  
        `throw new Error("Copilot model not found. Check subscription.");`  
    `}`  
    `const model = models;`

    `// 2. Check Consent`  
    `// Note: This does not trigger the UI, only checks state.`  
    `// The UI is triggered on the first actual sendRequest if not granted.`  
    `if (!model.access.canSendRequest) {`  
        `throw new Error("Consent not granted. Please approve in VS Code.");`  
    `}`

    `// 3. Construct Messages`  
    `const messages = [`  
        `vscode.LanguageModelChatMessage.User(userPrompt)`  
    `];`

    `// 4. Send Request & Accumulate Stream`  
    `const request = await model.sendRequest(messages, {}, new vscode.CancellationTokenSource().token);`  
      
    `let fullResponse = '';`  
    `for await (const token of request.text) {`  
        `fullResponse += token;`  
    `}`  
      
    `return fullResponse;`  
`}`

#### **Works cited**

1\. Get started with chat in VS Code, https://code.visualstudio.com/docs/copilot/chat/copilot-chat 2\. Getting started with chat in VS Code, https://code.visualstudio.com/docs/copilot/chat/getting-started-chat 3\. How to call GitHub Copilot Chat from my VS Code extension? \- Stack Overflow, https://stackoverflow.com/questions/77739243/how-to-call-github-copilot-chat-from-my-vs-code-extension 4\. Language Model API \- Visual Studio Code, https://code.visualstudio.com/api/extension-guides/ai/language-model 5\. VS Code API | Visual Studio Code Extension API, https://code.visualstudio.com/api/references/vscode-api 6\. Language Model Tool API | Visual Studio Code Extension API, https://code.visualstudio.com/api/extension-guides/ai/tools 7\. Copilot SemanticSearch tool for VSCode \- Reddit, https://www.reddit.com/r/vscode/comments/1odeb2v/copilot\_semanticsearch\_tool\_for\_vscode/ 8\. Programatically invoking \`copilot\_searchCodebase\` built-in tool using \`vscode.lm.invokeTool\` · community · Discussion \#156285 \- GitHub, https://github.com/orgs/community/discussions/156285 9\. Chat Participant API \- Visual Studio Code, https://code.visualstudio.com/api/extension-guides/ai/chat 10\. How to Build a Slackbot in Socket Mode with Python | Twilio, https://www.twilio.com/en-us/blog/developers/community/how-to-build-a-slackbot-in-socket-mode-with-python 11\. Using Socket Mode | Slack Developer Docs, https://docs.slack.dev/tools/bolt-python/concepts/socket-mode/ 12\. Telegram Bot API, https://core.telegram.org/bots/api 13\. project-nashenas-telegram-bot/Long Polling vs. Webhook.md at main \- GitHub, https://github.com/pytopia/project-nashenas-telegram-bot/blob/main/Long%20Polling%20vs.%20Webhook.md 14\. Test and debug your bot \- Teams | Microsoft Learn, https://learn.microsoft.com/en-us/microsoftteams/platform/resources/bot-v3/bots-test 15\. Debug your Teams app locally \- Agents Toolkit \- Microsoft Learn, https://learn.microsoft.com/en-us/microsoftteams/platform/toolkit/debug-local 16\. Developing with Remote Tunnels \- Visual Studio Code, https://code.visualstudio.com/docs/remote/tunnels