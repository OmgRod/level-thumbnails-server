export default [
    { path: '/', component: () => import('./pages/HomePage.vue') },
    { path: '/dashboard', component: () => import('./pages/DashboardPage.vue'), meta: { requiresAuth: true } },

]