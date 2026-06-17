import { useState } from "react";
import { createRoute } from "@tanstack/react-router";
import { invoke } from "@tauri-apps/api/core";
import { rootRoute } from "./__root.tsx";
import { Button } from "#components/ui/button.tsx";
import { Input } from "#components/ui/input.tsx";

function HomePage() {
  const [greetMsg, setGreetMsg] = useState("");
  const [name, setName] = useState("");

  async function greet() {
    setGreetMsg(await invoke("greet", { name }));
  }

  return (
    <div className="flex flex-col items-center gap-6">
      <h1 className="text-3xl font-bold">Welcome to Swiss Knife</h1>
      <form
        className="flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          greet();
        }}
      >
        <Input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Enter a name..."
          className="w-64"
        />
        <Button type="submit">Greet</Button>
      </form>
      {greetMsg && (
        <p className="text-muted-foreground">{greetMsg}</p>
      )}
    </div>
  );
}

export const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: HomePage,
});
