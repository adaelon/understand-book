import ReactDOM from "react-dom/client";
import { QueryClientProvider } from "@tanstack/react-query";
import {
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
  Navigate,
} from "@tanstack/react-router";
import { queryClient } from "./lib/api";
import { ThemeProvider } from "./context/theme-provider";
import { App } from "./app";
import { UsersPage, UserPage } from "./pages/users";
import { UsagePage } from "./pages/usage";
import { InvitesPage } from "./pages/invites";
import "./styles/index.css";

const root = createRootRoute({
  component: App,
  notFoundComponent: () => <p>页面不存在，请从账号管理进入。</p>,
});
const index = createRoute({
  getParentRoute: () => root,
  path: "/",
  component: () => <Navigate to="/users" replace />,
});
const login = createRoute({
  getParentRoute: () => root,
  path: "/login",
  component: () => null,
});
const users = createRoute({
  getParentRoute: () => root,
  path: "/users",
  component: UsersPage,
});
const user = createRoute({
  getParentRoute: () => root,
  path: "/users/$userId",
  component: () => {
    const { userId } = user.useParams();
    return <UserPage key={userId} userId={userId} />;
  },
});
const usage = createRoute({getParentRoute: () => root, path: '/usage', component: UsagePage});
const router = createRouter({
  routeTree: root.addChildren([index, login, users, user, usage, createRoute({getParentRoute: () => root, path: '/invites', component: InvitesPage})]),
  basepath: "/admin",
});
ReactDOM.createRoot(document.getElementById("root")!).render(
  <QueryClientProvider client={queryClient}>
    <ThemeProvider storageKey="ub-admin-theme">
      <RouterProvider router={router} />
    </ThemeProvider>
  </QueryClientProvider>,
);
