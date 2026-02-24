Setting up Slack tokens can feel like navigating a maze, especially since Slack uses several different types of tokens for different purposes.

Since you are running a Model Context Protocol (MCP) server, your server will likely need to connect to Slack via **Socket Mode**. This allows your local or firewalled MCP server to securely receive and send events without exposing a public HTTP endpoint.

To make this connection, you actually need **two** distinct things: the **App-Level Token** (to establish the websocket connection) and a **Bot Token** (to actually have the permission to send the messages).

Here is the correct, step-by-step way to set this up.

### 1. Generate the App-Level Token

This token (which always starts with `xapp-`) is what authorizes your MCP server to open a websocket connection with Slack.

1. Go to your [Slack API Apps Dashboard](https://api.slack.com/apps) and select your app.
2. In the left sidebar, click on **Basic Information**.
3. Scroll down to the **App-Level Tokens** section and click **Generate Token and Scopes**.
4. Give your token a descriptive name (e.g., `mcp-socket-connection`).
5. Click **Add Scope** and select `connections:write`. **This specific scope is mandatory** for Socket Mode to work.
6. Click **Generate**.
7. Copy the resulting string (starting with `xapp-`) and add it to your MCP server's environment variables, typically as `SLACK_APP_TOKEN`.

### 2. Enable Socket Mode

Creating the token doesn't automatically turn the feature on. You have to flip the switch.

1. In the left sidebar of your app settings, click on **Socket Mode**.
2. Toggle **Enable Socket Mode** to the "On" position.

### 3. Generate the Bot User OAuth Token

While the App-Level token connects your server to Slack, it doesn't give your bot permission to *do* anything inside the channels. To actually relay messages, you need a Bot Token (which always starts with `xoxb-`).

1. In the left sidebar, click on **OAuth & Permissions**.
2. Scroll down to **Scopes** -> **Bot Token Scopes**.
3. Click **Add an OAuth Scope**. At a bare minimum for relaying messages, you will need `chat:write`. You will also likely need `channels:history` and `channels:read` if your MCP server needs to read the context of the channel.
4. Scroll back up to the top of the **OAuth & Permissions** page and click **Install to Workspace** (or **Reinstall to Workspace** if you've already installed it).
5. Copy the **Bot User OAuth Token** (starting with `xoxb-`) and add it to your MCP server's environment variables, typically as `SLACK_BOT_TOKEN`.

---

**A quick tip on channel routing:** Remember that even with these tokens set up, your app cannot automatically post or listen to a channel until you explicitly invite it. Go to the specific Slack channel you want to use and type `/invite @[YourAppName]` to bring it in.
