import json
import os
from datetime import datetime

class Task:
    """Represents a single task in the system."""
    def __init__(self, task_id, title, description, due_date):
        self.task_id = task_id
        self.title = title
        self.description = description
        self.due_date = due_date  # Format: YYYY-MM-DD
        self.is_completed = False

    def mark_complete(self):
        """Changes the task status to completed."""
        self.is_completed = True

    def to_dict(self):
        """Converts task details into a dictionary for JSON saving."""
        return {
            "id": self.task_id,
            "title": self.title,
            "description": self.description,
            "due_date": self.due_date,
            "is_completed": self.is_completed
        }

    @classmethod
    def from_dict(cls, data):
        """Creates a Task object from dictionary data."""
        task = cls(data["id"], data["title"], data["description"], data["due_date"])
        task.is_completed = data["is_completed"]
        return task


class TaskManager:
    """Manages a list of tasks and handles saving/loading."""
    def __init__(self, storage_file="tasks.json"):
        self.storage_file = storage_file
        self.tasks = self._load_tasks()

    def add_task(self, title, description, due_date):
        """Adds a new task with a unique ID."""
        # Simple data validation
        try:
            datetime.strptime(due_date, "%Y-%m-%d")
        except ValueError:
            print("❌ Invalid date format! Please use YYYY-MM-DD.")
            return False

        task_id = len(self.tasks) + 1
        new_task = Task(task_id, title, description, due_date)
        self.tasks.append(new_task)
        self._save_tasks()
        print(f"✅ Task '{title}' added successfully!")
        return True

    def complete_task(self, task_id):
        """Finds a task by ID and marks it as done."""
        for task in self.tasks:
            if task.task_id == task_id:
                task.mark_complete()
                self._save_tasks()
                print(f" Complete: Task {task_id} is done!")
                return True
        print("❌ Task ID not found.")
        return False

    def list_tasks(self):
        """Prints all tasks, highlighting overdue ones."""
        if not self.tasks:
            print("📂 No tasks found.")
            return

        today = datetime.now().date()
        print("\n--- CURRENT TASKS ---")
        for task in self.tasks:
            status = "✅ Done" if task.is_completed else "⏳ Pending"

            # Check if overdue
            task_date = datetime.strptime(task.due_date, "%Y-%m-%d").date()
            if not task.is_completed and task_date < today:
                status = "🚨 OVERDUE"

            print(f"[{task.task_id}] {task.title} - Due: {task.due_date} ({status})")
            print(f"    Description: {task.description}")
        print("----------------------\n")

    def _save_tasks(self):
        """Saves tasks to a JSON file."""
        with open(self.storage_file, "w") as file:
            json.dump([task.to_dict() for task in self.tasks], file, indent=4)

    def _load_tasks(self):
        """Loads tasks from a JSON file if it exists."""
        if not os.path.exists(self.storage_file):
            return []
        try:
            with open(self.storage_file, "r") as file:
                data = json.load(file)
                return [Task.from_dict(item) for item in data]
        except (json.JSONDecodeError, KeyError):
            print("⚠️ Warning: Storage file corrupted. Starting with empty list.")
            return []


# --- Text-Based User Interface ---
def main():
    manager = TaskManager()

    while True:
        print("1. View Tasks")
        print("2. Add Task")
        print("3. Complete Task")
        print("4. Exit")
        choice = input("Choose an option (1-4): ").strip()

        if choice == "1":
            manager.list_tasks()
        elif choice == "2":
            title = input("Enter task title: ")
            desc = input("Enter task description: ")
            date = input("Enter due date (YYYY-MM-DD): ")
            manager.add_task(title, desc, date)
        elif choice == "3":
            try:
                task_id = int(input("Enter Task ID to complete: "))
                manager.complete_task(task_id)
            except ValueError:
                print("❌ Please enter a valid number ID.")
        elif choice == "4":
            print("Goodbye!")
            break
        else:
            print("❌ Invalid choice. Please pick 1, 2, 3, or 4.")

if __name__ == "__main__":
    main()
1
