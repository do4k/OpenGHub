using System;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Platform;
using OpenGHub.App.ViewModels;

namespace OpenGHub.App.Views;

public partial class MainWindow : Window
{
    private readonly MainWindowViewModel _vm = new();

    public MainWindow()
    {
        Icon = new WindowIcon(AssetLoader.Open(new Uri("avares://OpenGHub.App/Assets/app-icon.png")));
        InitializeComponent();
        DataContext = _vm;
        Closed += (_, _) => _vm.DisposeAsync().AsTask().GetAwaiter().GetResult();
    }
}