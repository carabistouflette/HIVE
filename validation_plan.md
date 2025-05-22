# Plan de Validation : Création et Exécution de Tâches Simples avec un Seul Agent

Ce plan de validation vise à garantir que l'application peut correctement créer, configurer et exécuter des tâches simples en utilisant un seul agent d'IA, et que les résultats sont conformes aux attentes.

## 1. Méthodologie de Test Structurée

Nous adopterons une approche de test fonctionnel et d'intégration, en se concentrant sur le flux de bout en bout de la création à l'exécution d'une tâche simple.

*   **Tests Manuels** : Exécution pas à pas des cas d'usage pour observer le comportement de l'interface utilisateur et les résultats.
*   **Tests d'Intégration** : Vérification de la bonne communication entre les composants (UI, `CoreOrchestrator`, `AgentManager`, `ExternalApiClient`, `MCPManager`, `Persistence`).
*   **Tests de Régression** : Après toute modification, s'assurer que les fonctionnalités existantes ne sont pas affectées.
*   **Journalisation et Suivi** : Utilisation des logs de l'application pour diagnostiquer les problèmes et vérifier les étapes d'exécution.

## 2. Cas d'Usage à Couvrir

Les cas d'usage se concentreront sur la création et l'exécution réussie d'une tâche simple, ainsi que sur la gestion des erreurs de base.

### CU1 : Création et Exécution Réussie d'une Tâche Simple
*   **Description** : L'utilisateur crée une nouvelle tâche simple, assigne un agent existant (ou un agent par défaut si applicable), configure les paramètres de base (prompt, modèle LLM), lance l'exécution, et vérifie que la tâche se termine avec succès et produit le résultat attendu.
*   **Préconditions** :
    *   L'application est lancée et fonctionnelle.
    *   Une connexion à l'API OpenRouter est établie (clé API valide configurée).
    *   Au moins un agent est disponible ou peut être créé.
*   **Étapes** :
    1.  Lancer l'application.
    2.  Naviguer vers l'interface de création de tâche.
    3.  Saisir un prompt clair et simple pour l'agent (ex: "Écris une courte phrase sur les chats.").
    4.  Sélectionner un agent (ex: `WriterAgent` ou `SimpleWorker`).
    5.  Sélectionner un modèle LLM compatible avec OpenRouter (ex: `mistralai/mistral-7b-instruct`).
    6.  Lancer l'exécution de la tâche.
    7.  Observer la progression de la tâche (statut : `Pending` -> `Running` -> `Completed`).
    8.  Vérifier le résultat généré par l'agent.
    9.  Vérifier que le résultat est persistant (si applicable, après redémarrage de l'application).

### CU2 : Gestion des Erreurs - Clé API Invalide
*   **Description** : L'utilisateur tente d'exécuter une tâche avec une clé API OpenRouter invalide ou manquante, et l'application gère l'erreur de manière appropriée.
*   **Préconditions** :
    *   L'application est lancée.
    *   La clé API OpenRouter est configurée comme invalide ou est absente.
*   **Étapes** :
    1.  Configurer une clé API OpenRouter invalide ou la supprimer.
    2.  Tenter de créer et d'exécuter une tâche simple comme dans CU1.
    3.  Vérifier que l'application affiche un message d'erreur clair et ne plante pas.
    4.  Vérifier que la tâche échoue avec un statut approprié (ex: `Failed`).

### CU3 : Gestion des Erreurs - Modèle LLM Indisponible/Invalide
*   **Description** : L'utilisateur tente d'exécuter une tâche avec un modèle LLM qui n'est pas supporté par OpenRouter ou est mal orthographié.
*   **Préconditions** :
    *   L'application est lancée.
    *   Une clé API OpenRouter valide est configurée.
*   **Étapes** :
    1.  Tenter de créer et d'exécuter une tâche simple en sélectionnant un modèle LLM invalide (ex: `nonexistent/model` ou un modèle qui n'est pas sur OpenRouter).
    2.  Vérifier que l'application affiche un message d'erreur clair et ne plante pas.
    3.  Vérifier que la tâche échoue avec un statut approprié (ex: `Failed`).

## 3. Critères de Succès

Pour chaque cas d'usage, les critères de succès suivants doivent être remplis :

*   **Fonctionnalité** : La tâche est créée, exécutée et complétée comme prévu.
*   **Résultat** : Le résultat généré par l'agent est pertinent et conforme au prompt.
*   **Statut de la Tâche** : Le statut de la tâche (`Pending`, `Running`, `Completed`, `Failed`) est mis à jour correctement et reflète l'état réel.
*   **Gestion des Erreurs** : Les erreurs sont détectées, signalées à l'utilisateur de manière claire et l'application reste stable.
*   **Persistance** : Les informations de la tâche (prompt, agent, modèle, résultat, statut) sont correctement stockées et récupérables après un redémarrage de l'application.
*   **Performance** : L'exécution de la tâche se fait dans un délai raisonnable (pour une tâche simple).

## 4. Informations Essentielles Nécessaires à une Simulation Réaliste

Pour exécuter ces tests de manière réaliste, les informations suivantes sont cruciales :

*   **Preuves Visuelles** :
    *   Captures d'écran de l'interface utilisateur à chaque étape clé (création de tâche, exécution, affichage du résultat, messages d'erreur).
    *   Enregistrements vidéo du flux de travail complet pour les cas d'usage critiques.
*   **Informations d'Identification** :
    *   **Clé API OpenRouter** : Une clé API valide pour les tests réussis. Une clé API invalide/vide pour les tests d'erreur.
*   **Configurations Techniques** :
    *   **Modèle LLM** : Nom exact du modèle LLM à utiliser (ex: `mistralai/mistral-7b-instruct`).
    *   **Paramètres du LLM** : Température, max_tokens, etc., si l'interface utilisateur permet leur configuration. Pour les tests simples, les valeurs par défaut peuvent suffire.
    *   **Prompts de Test** : Liste de prompts spécifiques et simples à utiliser pour chaque cas d'usage.
*   **Données Pertinentes** :
    *   **Logs de l'Application** : Accès aux logs de la console ou du fichier pour vérifier les appels API, les erreurs internes et la progression de l'orchestration.
    *   **Contenu de la Base de Données** : Vérification directe de la base de données (si possible via un outil de DB) pour confirmer la persistance des données de la tâche.

## 5. Diagramme de Flux de Test (Mermaid)

```mermaid
graph TD
    A[Démarrer l'Application] --> B{Interface de Création de Tâche};
    B --> C{Saisir Prompt & Sélectionner Agent/Modèle};
    C --> D{Lancer l'Exécution de la Tâche};
    D -- Succès --> E[Tâche en Cours (Running)];
    E --> F[Tâche Terminée (Completed)];
    F --> G{Vérifier Résultat & Persistance};
    D -- Échec (API/Modèle Invalide) --> H[Tâche Échouée (Failed)];
    H --> I{Vérifier Message d'Erreur & Stabilité};
    G --> J[Validation Réussie];
    I --> K[Validation Réussie (Gestion Erreur)];