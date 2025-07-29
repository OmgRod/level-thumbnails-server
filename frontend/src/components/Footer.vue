<script setup lang="ts">

</script>

<template>
  <footer class="footer">
    <p>&copy; {{ new Date().getFullYear() }} Level Thumbnails.</p>
  </footer>
</template>

<style scoped>
.footer {
  text-align: center;
  padding: 20px;
  background-color: rgba(0, 0, 0, 0.5);
  color: white;
}

.footer a {
  color: #00aaff;
  text-decoration: none;
}

.footer a:hover {
  text-decoration: underline;
}

.footer p {
  margin: 0;
  font-size: 0.9em;
}
</style>