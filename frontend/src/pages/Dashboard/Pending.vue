<script setup lang="ts">
import {ref} from "vue";
import LoadingCircle from "../../components/LoadingCircle.vue";

const loading = ref(true);
const error = ref<string | null>(null);
(async () => {
    try {
        const response = await fetch('/pending');
        const data = await response.json();

        // Check if the response is ok
        if (!response.ok) {
            throw new Error(data.message || 'Failed to fetch pending items');
        }

        console.log(data);
    } catch (err) {
        error.value = err instanceof Error ? err.message : 'An unknown error occurred';
    } finally {
        loading.value = false;
    }
})();

</script>

<template>
  <div v-if="loading" class="d-flex flex-middle h-100">
    <LoadingCircle />
  </div>
  <div v-else-if="error" class="error-message">
    <img src="/error.svg" alt="Error Icon" style="width: 128px; height: auto;"/>
    <p>{{ error }}</p>
  </div>
  <div v-else class="error-message">
    <img src="/error.svg" alt="Error Icon" style="width: 128px; height: auto;"/>
    <p>Oops. Not implemented yet</p>
  </div>
</template>

<style scoped>
.error-message {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  text-align: center;
  font-size: 1.2em;
}

</style>