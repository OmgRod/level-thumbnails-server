<script setup lang="ts">
import SessionManager from "../managers/session.ts";
import LoadingCircle from "../components/LoadingCircle.vue";
import {ref} from "vue";

const user = ref(SessionManager.getUser());
if (!user.value) {
  SessionManager.validateSession().then(()=>{
    user.value = SessionManager.getUser();
  }).catch((error) => {
    console.error("Session validation failed:", error);
    SessionManager.logout();
  })
}
</script>

<template>
  <LoadingCircle backdrop v-if="!user"/>
  <main v-else>
    <h1>Dashboard</h1>
    <p>
      You are <strong>{{ SessionManager.getAuthRole() }}</strong>
      {{ user }}
    </p>
  </main>
</template>

<style scoped>

</style>