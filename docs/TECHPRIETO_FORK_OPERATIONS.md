# Operación del fork TechPrieto/buzz

Esta guía explica cómo cuidamos la versión de Buzz que usa TechPrieto sin
pedirle a Abraham que administre GitHub ni que tome decisiones de desarrollo.

## El mapa

```text
block/buzz       → El Buzz original mantenido por Block.
      ↓
TechPrieto/buzz  → Nuestra copia de trabajo (fork), con ajustes aprobados.
      ↓
VPS TechPrieto   → El entorno que usa el equipo.
```

Un **fork** es una copia controlada del código original. Nos permite resolver
necesidades de TechPrieto sin esperar a que Block las priorice. No reemplaza al
proyecto original, ni nos obliga a instalar cada novedad que Block publique.

`main` significa “la versión principal” dentro de un repositorio. Por eso hay
dos referencias distintas:

- `block/buzz → main`: versión principal de Block.
- `TechPrieto/buzz → main`: nuestra referencia estable de producción.

## Qué decide Abraham y qué ejecuta el equipo técnico

Abraham decide el resultado de negocio y aprueba publicaciones a producción.
El equipo técnico decide la mecánica: ramas, comandos, pruebas, compatibilidad
y documentación. Los reportes para Abraham deben responder solo estas preguntas:

1. ¿Qué mejora o riesgo estamos atendiendo?
2. ¿Qué notará el equipo si se publica?
3. ¿Qué se probó?
4. ¿Qué podría salir mal y cómo se vuelve atrás?
5. ¿Recomendamos publicar o no publicar?

Nunca se le debe pedir a Abraham que escoja un commit, una rama, un rebase o un
comando de Git.

## Cómo trabajamos

### Cambios propios

Todo cambio para TechPrieto se hace en una rama aislada y se revisa en un PR.
La versión estable nunca se modifica directamente. El cambio se anota en
[`PATCHES.md`](../PATCHES.md) con su propósito, estado, pruebas y el plan para
reemplazarlo por una solución oficial si Block publica una equivalente.

Antes de crear una adaptación nueva, el equipo verifica si Buzz oficial ya la
resuelve. Si la mejora podría ayudar a cualquier equipo de Buzz, se propone a
upstream para que no tengamos que mantenerla indefinidamente.

### Actualizaciones desde Block

No copiamos actualizaciones oficiales automáticamente ni las ignoramos.
Periódicamente —y siempre ante una corrección de seguridad— el equipo compara
la versión oficial con la nuestra y propone una actualización concreta.

La actualización se prepara en una versión de prueba, no sobre producción. Se
integran primero nuestros ajustes existentes, se resuelven conflictos, se
prueba todo y solo después se pide autorización para publicar. Antes de empezar,
se guarda un punto de regreso verificable de la versión que hoy funciona.

No se integra código cambiante sin identificar: cada actualización usa una
versión o commit específico de Block, documentado en el PR.

### Publicación segura

Antes de publicar, el equipo debe:

1. Ejecutar las pruebas apropiadas para lo que cambió.
2. Probar la actualización y las migraciones de base de datos fuera de
   producción cuando aplique.
3. Revisar alertas de seguridad y escanear el historial para evitar secretos.
4. Tener un rollback claro al punto de regreso anterior.
5. Entregar a Abraham una recomendación clara: **publicar** o **no publicar**.

Una rama verde o un PR aprobado no son autorización automática para cambiar el
VPS. La publicación requiere la aprobación explícita de Abraham.

## Reglas de protección

- Nunca subir claves, tokens, archivos `.env`, respaldos de producción ni
  material de recuperación a GitHub.
- Nunca reescribir la historia de la versión estable ni hacer force-push.
- Los cambios de relay, esquema de base de datos, migraciones o tipos de evento
  Nostr requieren pruebas de actualización y rollback antes de producción.
- No construir una función grande sobre una base que ya está en proceso de
  actualización, salvo que Abraham priorice esa función de forma explícita.

## Estado y próximos pasos

El registro técnico de las personalizaciones vigentes está en
[`PATCHES.md`](../PATCHES.md). El próximo ciclo recomendado es una integración
controlada y pruebas de hardening; después se implementa el estado personal y
compartido de los hilos sobre esa base validada.
